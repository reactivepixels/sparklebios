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
- Mane, a fifth theme variant built from the unicorn's own colours, made to sit under `pc95`. Opt in with `theme = rainbows-and-unicorns-mane`.
- Four more theme variants: Miami, Arcade, VHS and Den. Switch with `bios theme use <name>`.
- A cursor trail shader for Ghostty: when the cursor jumps it leaves a short six-stripe trail. Opt in.
- Tool colours: `ls`, `eza`, `bat`, `fzf`, `man`, `grep`, zsh completion and `delta` told to use the terminal's own sixteen colours, so they follow every theme variant.
- `extras/starship-palette.toml`, a starship palette to match the theme.
- `bios init zsh`, `bios machines`, and the machine authoring guide in `docs/machines.md`.
- Health checks: the boot screen now reports three real things about your machine. Boot device order lists the projects you were most recently working in and says when you walked away from the first one mid-change, and `bios resume` takes you back to it. IRQ conflicts names a dev server you forgot about that is still holding a well known port. The virus scan reports files that git is tracking and that look like secrets or keys. Findings are phrased per flavour, and a Fail is shown even during a burst of new tabs, as a single line.
- A fact cache with a detached refresh. No probe ever runs while a shell is starting: the boot path reads one JSON file and draws, then spawns `bios refresh` and never waits on it. A finding stops being shown once it is older than its own ttl, so the boot device list survives overnight and the port result does not. `checks = false` turns the whole thing off. The checks are documented in `docs/checks.md`.
- Five more flavours: `ninja`, `viking`, `luchador`, `yeti` and `raccoon`, each with its own mascot and quips. The roster is now seven.
- A tenth theme variant, Sorbet. Switch with `bios theme use sorbet`.
- `graphics` in `config.toml`: `auto` (the default, today's behaviour), `image` (the same choice, named explicitly), or `blocks`, which always draws the half-block mascot even in a Kitty-capable terminal, for terminals that drop the image when a tab goes to sleep. `SPARKLEBIOS_GRAPHICS` overrides it. The half-block mascot also gained a bigger, 28 by 28 grid for every flavour, drawn instead of the original 14 by 14 one whenever the screen is wide enough for it beside the logo; `pc95` stays on the smaller one.
- `bios theme use <name>` now also points an existing starship prompt at a matching palette from `extras/starship-palette.toml`, `rainbows_and_unicorns_paper` for Paper White and `rainbows_and_unicorns_auto` for every other variant. It only touches a starship config that already has a top level `palette =` line, never creates one, and `--no-prompt` skips it.

### Changed

- A finding line too wide for the screen now shortens the long value inside it, marked with two full stops, rather than cutting the end off the sentence. The repository name gives way before the filename, and the instruction at the end of the line always survives.
- The README now opens with real boot screens for both flavours and all ten themes, rendered from the binary's own output.

- One screen. The `pc85` and `c64` machines are retired; every boot is `pc95` and flavours carry the personality. The once-a-day boot is animated, the rest are instant.
- `bios use <flavour>` replaces the machine options, `bios theme use <name>` switches Ghostty's theme, and `bios --help` is written by hand.
