# Changelog

All notable changes to SparkleBIOS are recorded here, in the manner of a service
log kept in a binder next to the machine.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

What 0.1.0 ships, grouped by part.

### Boot screen

- One screen, `pc95`, drawn from a flavour: the mascot, the firmware and vendor wording, one detected part, the streak line, the footer code and the quips.
- The first boot of the day plays the show over a couple of seconds; every boot after that draws the finished screen at once. Any key skips to the end, and whatever you typed is waiting on your prompt.
- Painted screens: a machine can draw itself as a block of its own colour with a border, or sit transparently on the terminal's own background. The mascot is a real image, through the Kitty or iTerm2 protocol, wherever the terminal supports one.
- The boot screen notices the date. Pi Day, the 256th day of the year, Friday the 13th and a handful of others get a line of their own in place of the usual quip.
- A boot-day streak, shown on the screen and, after two days, in the prompt mascot too.
- `SPARKLEBIOS_BOOT=0`, the `boot` config key, and `bios boot --no-animate` all turn the screen off or still, without touching the shell.

### Health checks

- Eight checks, each cached and phrased by your flavour: boot device order, IRQ conflicts, the virus scan, disk trend, changed dotfiles, stale stashes, runtime drift and battery health.
- No probe ever runs on the boot path. `bios refresh` runs them in the background and the boot screen reads the result from a cache.
- `bios resume` takes you back to the project you left work in.

### Flavours

- Eight flavours ship: `unicorn` (the default), `sumo`, `ninja`, `viking`, `luchador`, `yeti`, `raccoon` and `wizard`, each with its own mascot, wording and quips.
- `bios flavours`, `bios use <flavour>`, `bios boot --flavour <id>`.
- `bios flavour new <id>` writes a starter flavour file, ready to boot before you have changed a word.

### Themes

- "Rainbows and Unicorns", a Ghostty theme in ten variants, with a test that fails if a theme file and its documentation ever disagree.
- `bios theme list`, `bios theme install` and `bios theme use <name>`, which also points an existing starship prompt at a matching palette from `extras/starship-palette.toml`.
- A cursor trail shader, terminal-following tool colours for `ls`, `eza`, `bat`, `fzf`, `man`, `grep` and `delta`, all optional, in `extras/`.

### Setup

- `bios setup`, the CMOS Setup Utility: one blue screen for every option, filling the whole terminal in a fixed CGA blue rather than sitting on your theme. Arrow keys move and change, Enter previews the boot screen with your pending choices, F10 saves, Esc leaves. Nothing is written until you save.
- `bios config path`, `bios config edit` and `bios config reset` read and write the same config file by hand, preserving comments, blank lines and key order.
- `NO_COLOR` is honoured throughout: the boot screen, `bios fetch` and `bios setup` alike.

### Presence

- The flavour keeps talking after the boot screen: it sets your tab title, comments on a command that ran long, answers a typo, and says goodbye when the shell exits. Nothing here spawns a process; the shell already holds every word it will say.
- The mascot rides in the prompt too, with the boot streak after it.

### Sprinkles

- An optional effects layer on the once-a-day show, off by default: `light` (a shimmer, a twinkle, and a stripe sweep on a streak milestone) or `full` (the same, plus the POST beep and beep codes). Never flashes, and costs nothing when off.

### Shell

- `bios init zsh`, `bios init bash` and `bios init fish` install the hook. Typeahead is preserved on every one; bash reads and discards what was typed during the show rather than faking a replay, since it has no way to push it back this early in startup.

### Fetch

- `bios fetch` prints the machine at a glance: the mascot, a column of facts and the palette. The screen you paste into a thread.

### Speed

- The README's speed number is a receipt, not a claim. `tools/speed_receipt.py` drives a real hook on a pseudo-terminal and measures it on every push, and the build fails if a new tab or a skipped boot goes over budget.

### Release plumbing

- POST, the continuous integration workflow: format, lint, tests, the em-dash and en-dash grep, and `cargo-deny` for licences, advisories and the no-network rule, on macOS and Linux.
- A dependency policy that bans network crates outright.
- The code of conduct, the social preview artwork, and the pixel unicorn mark.
