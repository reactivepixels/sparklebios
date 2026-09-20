# SparkleBIOS: project design

Date: 2026-09-20
Status: accepted

## 1. What this is

A retro terminal toolkit with a 1985 sensibility. Serious engineering under an
unserious surface. It has three parts that share one binary, one config and one
visual language:

| Part | What it does | Role |
|---|---|---|
| **The BIOS** | Boots every new terminal tab with a period-correct POST screen that is secretly a health check | The star |
| **neigh** | Planned rainbow pipe with quantised era palettes (`make \| neigh`); not built yet, see `ROADMAP.md` M4.5 | The paint |
| **The theme** | "Rainbows and Unicorns", a serious 1985 Ghostty theme, ten variants sharing one idea | The ground |

Parked for later, per `ROADMAP.md`: **stable** (a unicorn that lives in the
prompt, with commit streaks) and exit code theatre.

### Principles

1. **Be cool and hilarious.** That is the mandate. Deadpan delivery, period-correct detail, gags that reward people who remember the real thing. `docs/voice.md` is the rulebook. Everything below is how we get away with it.
2. **Never slow or break the shell.** Every feature has a time budget and a kill switch. A failure prints nothing and exits 0.
3. **Jokes carry facts.** Every gag line is backed by a real probe or is clearly fictional branding.
4. **Quantised, not gradient.** Bands, dither and raster bars, the design for `neigh` once it exists. No smooth truecolor blends.
5. **Each flavour speaks in its own voice.** A unicorn, a sumo wrestler and a raccoon phrase the same warning differently. The screen underneath (`pc95`) is the hardware; the flavour is the personality riding on it.
6. **Content is data.** Screens live in `machines/*.toml`, flavours in `flavours/*.toml`, themes in `themes/`. Logic never hardcodes a screen's or a flavour's wording.
7. **The name is the only joke in the theme.** The theme itself is a daily driver with checked contrast.

## 2. Command surface

One Rust binary, `bios`. Its `--help` (also shown for a bare `bios` and `bios
help`) is hand written rather than clap generated:

```
SparkleBIOS {version}
A 1995 POST screen for your terminal that is secretly a health check.

Usage: bios <COMMAND>

Everyday:
  boot               Play the boot screen now
  resume             Change to the project you left work in
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
  bios resume                  Go back to the project you left work in
  bios use                     Show which flavour is set
  bios theme use mane          Switch Ghostty to the Mane theme
  SPARKLEBIOS_BOOT=0           Set this in a shell to stop it booting there

Options:
  -h, --help         Print help
  -V, --version      Print version
```

`bios refresh` also exists (rebuilds the findings cache, spawned detached by
`bios boot` and never meant to be typed by hand). `bios boot` also takes a
hidden `--machine <id>` for previewing a screen file under development.

Shell support: zsh only today. bash and fish hooks are `ROADMAP.md` M8;
nothing in the binary is zsh specific.

## 3. The BIOS

### 3.1 Boot modes

SparkleBIOS ships one screen, `pc95` (see 3.5). A boot always draws that
screen; the mode only decides whether it animates and how much of it shows.

| Mode | When | What happens |
|---|---|---|
| **Full show** | First boot of the day | `pc95`, played out over a couple of seconds with the timeline's own timing, skippable by any key |
| **Fast** | Every other new tab, same day | The same screen, drawn at once, no animation |
| **Quiet** | A boot happened inside the burst window (10s: a burst of tabs, an editor or agent spawning many shells) | Nothing, unless a `Fail` finding is fresh in the cache, in which case one plain line, no paint, no colour, no F1 line |
| **Off** | See below | Nothing |

Off conditions: stdout is not a tty, `TERM=dumb`, `SPARKLEBIOS_BOOT=0`, or
`SPARKLEBIOS_BOOTED` is already exported (a nested shell). The zsh hook itself
only calls `bios boot --hook` when the shell is interactive
(`[[ -o interactive ]]`), so a non-interactive shell never gets this far.
There is no user-configured skip rule (for example by `TERM_PROGRAM`) today.

The final POST stays in scrollback like a MOTD. It is not cleared.

### 3.2 The timeline

A screen is a list of steps, played in order:

```
Step = Print{text, style, ms} | Count{template, to, suffix, ms}
     | Detect{label, result, style, ms} | Quip{style, ms}
     | Findings{style, ms} | F1{style, ms}
```

- `render::render_static` draws the final frame at once: this is what Fast
  mode, a non-TrueColor terminal, `--no-animate` and `SPARKLEBIOS_ANIMATE=0`
  all use.
- `show::play` is the animated player, used only on a Full decision. It walks
  the same steps with their own delays and watches for a key on `/dev/tty`.
  Tests play it at speed 0 and assert the final rows equal `render_static`'s
  output, so the two paths can never silently disagree.
- **Any key** finishes the show instantly and jumps to the final frame. There
  is no dedicated setup screen to open with a different key yet (`bios setup`
  is `ROADMAP.md` M5).
- **Typeahead is preserved.** The binary reads keys from `/dev/tty`, and
  `--hook` mode prints the swallowed bytes (escape sequences and Ctrl-C
  filtered out) to stdout; the shell hook pushes them into the line editor
  with `print -z`.
- Raw mode is entered through a guard that restores the terminal's saved
  termios when it drops, including during a panic. The guard also turns
  `ISIG` off, so Ctrl-C during the show arrives as a plain byte (`0x03`) like
  any other key, rather than a signal: there is no separate SIGINT handler to
  keep it from leaving the terminal raw.

### 3.3 Facts

Facts fill a screen's `{key}` slots. `facts::gather()` reads all of them
inline, every boot: CPU and memory via sysctl, disk usage via statfs, the
shell's own start time via `proc_pidinfo` on the parent pid. There is no
separate slow-fact cache: gathering is fast enough to test for (under 50ms on
macOS) that it never needs one. `shell.boot_ms` is absent once the shell has
been running more than 10 seconds, so a stale figure is never shown. The full
fact list, one row per key, is in [machines.md](machines.md).

A cold cache (no `facts.json` yet) shows no findings on the first boot at
all; see 3.4.

### 3.4 Checks

A check is `{id, probe, ttl, severity}` and yields a finding. Three exist
today:

| Check | What it reports |
|---|---|
| Boot device order | The git repositories touched most recently, and whether the most recent one has uncommitted changes |
| IRQ conflicts | A TCP listener on a well known dev port that has been open a long time |
| Virus scan | Files git is tracking in a boot device that look like secrets or keys |

The rest of the check set (disk trend, dotfiles changed, stale stashes,
battery, runtime drift) is planned, not built: see `ROADMAP.md` M3.5. Full
detail on probes, commands, allow lists and the cache is in
[checks.md](checks.md); this section only states the design intent.

No probe ever runs on the boot path. `bios refresh` is a separate command,
spawned detached by `bios boot` and never waited on, and it is the only thing
that runs a probe. The boot path itself reads one JSON cache file
(`facts.json`: a `generated` timestamp and the last findings) and nothing
else. A finding stops being shown once it is older than its own ttl,
independent of the cache's own age, so a slow-moving result (the boot device
list) survives overnight while a fast-moving one (an open port) does not. On
a cold cache the boot shows nothing; that boot's spawned refresh writes the
cache, and the next boot reads what it found.

Any `Warn` or `Fail` ends the POST with an F1 line (theatre: it never
actually waits for a key). A `Fail` is shown even in Quiet mode, as one line.

### 3.5 The screen

SparkleBIOS ships one screen, `pc95`, a mid-90s Award-style POST. It draws
its personality (mascot, firmware and vendor wording, one detected part, the
streak line, the footer code and its quips) from the current **flavour**
rather than hardcoding it (`flavoured = true`). Seven flavours ship built in:
`unicorn` (the default), `sumo`, `ninja`, `viking`, `luchador`, `yeti` and
`raccoon`. Both the screen and flavour formats are plain TOML, embedded at
compile time and overridable from `~/.config/sparklebios/machines/` and
`~/.config/sparklebios/flavours/`, so nothing about adding one needs a
recompile of logic.

A finding's phrasing is looked up in the current flavour's `[findings]`
table first, and falls back to the screen's own table for any id the flavour
does not carry. `unicorn` deliberately ships no `[findings]` table at all, so
it inherits the screen's wording unchanged: a flavour only needs to write the
lines where its voice actually differs.

The full schema (steps, slots, the omitted-line rule, how quips rotate,
painted screens, graphics) is in [machines.md](machines.md); the flavour
schema and its sprite pair is in [flavours.md](flavours.md). A trimmed real
excerpt from `pc95.toml`:

```toml
[[step]]
print = "{flavour.firmware}"
style = "bright"

[[step]]
count = "Memory Testing : {n}K"
to = "{mem.kb}"
suffix = " OK"
ms = 600

[[step]]
detect = "Detecting {flavour.part}"
result = "{flavour.part_result}"

[findings]
boot_order = "Boot device order: {boot.devices}"
f1_resume = "Press F1 to continue, or bios resume to boot {boot.device}."
```

All pixel art and wording is original. No real logos or wordmarks ship in the
product (see [machines.md](machines.md#originality)).

### 3.6 Sound, shutdown, calendar

Planned, not built: beep codes on the once-a-day full show, a shutdown line
on the zshexit hook, and calendar gags (Friday the 13th, 01-01-1980, the
project's own birthday). See `ROADMAP.md` M3.5.

## 4. Project POST

Planned, not built. The design intent is a `chpwd` hook that prints a short
mini POST in the active flavour's voice on entering a repo root, with
zero-config checks (node version against `.nvmrc`, `.env` against an example
file, Docker running when a compose file exists, a repo's own Supabase
binding) and custom checks from an opt-in `.bios.toml`, trusted per directory
with `bios trust`. See `ROADMAP.md` M6 for the milestone this belongs to.

## 5. neigh

Planned, not built. The design intent is a rainbow pipe (`make | neigh`) in
the same quantised, era-correct palettes as the BIOS: bands, a stepped
diagonal, dither, raster bars, a flash mode, a banner and rule renderer the
BIOS's own painter would share. See `ROADMAP.md` M4.5. No flag surface is
fixed yet, so none is written down here.

## 6. The theme

Ten Ghostty theme files share one idea: the rainbow is simply the ANSI
colours your tools were already going to use. Exact palettes, the reasoning
behind each variant and the contrast figures are in [theme.md](theme.md).
`bios theme install` copies them into the Ghostty user themes directory;
`bios theme use <name>` also switches Ghostty's config to one and, where a
starship config already opts in, points its palette at the match. Neither
ever edits a config file that was not already asking for one.

## 7. Setup utility

Planned, not built. The design intent is a `bios setup` screen (ratatui,
arrow keys, Enter, Esc, F10 to save) editing the same `config.toml` a person
could edit by hand, plus a beige fail-safe mode (no animation, no sound, no
colour, checks only). See `ROADMAP.md` M5.

## 8. Files on disk

| Path | Purpose |
|---|---|
| `~/.config/sparklebios/config.toml` | Settings: `animate`, `flavour`, `checks`, `project_dirs`, `graphics` |
| `~/.config/sparklebios/machines/`, `flavours/` | User screen and flavour overrides and additions |
| `~/.local/state/sparklebios/state.json` | Last boot time, last full-show day, boot-day streak |
| `~/.cache/sparklebios/facts.json` | The findings cache: when it was generated, and the findings from that run |

## 9. Architecture

Single crate, library plus a thin binary, module boundaries chosen so each
can be tested alone:

```
src/
  main.rs        entry point: catches panics, sets the exit code policy
  lib.rs         module declarations only
  cli.rs         clap definitions and dispatch
  boot.rs        the boot flow: decide, gather facts, render or animate, print, save state
  mode.rs        the BootMode decision, a pure function
  show.rs        the animated show: plays a screen's steps, any key skips to the end
  render.rs      static rendering and colour modes
  machine.rs     screen TOML schema, validation, built-ins, lookup
  flavour.rs     flavour TOML schema, validation, built-ins, lookup, slot application
  sprite.rs      boot logos: half-block grids and Kitty images, one source image per sprite
  template.rs    {slot} substitution
  facts/         inline fact probes: macos.rs (real), other.rs (stub, Linux is M8)
  checks/        the three health checks (ports, projects, secrets) and the Finding type
  cache.rs       the findings cache read on the boot path
  state.rs       boot state: last boot, last full-show day, streak
  config.rs      user config
  theme.rs       embedded Ghostty theme files and install
  shell.rs       embedded shell hook text
  tty.rs         raw terminal mode and key polling
  term.rs        terminal geometry
  clock.rs       local date helpers over libc
  paths.rs       XDG directory resolution
machines/  flavours/  sprites/  themes/  shell/  extras/   embedded data
```

Data flow for a boot: `decide mode -> load state -> gather facts (inline) ->
load the findings cache, when checks are on -> resolve the flavour and apply
its slots -> render or play pc95 -> write typed keys back to the hook, save
state -> spawn bios refresh detached, only when the cache is stale`.

### Budgets and safety rules

| Path | Budget |
|---|---|
| Off or quiet decision | under 5ms |
| Work before first frame | under 30ms |

- `bios boot` and `bios refresh` always exit 0 and print nothing unless
  `SPARKLEBIOS_DEBUG=1`. Other commands, typed by a person rather than run on
  every shell start, can fail loudly (`bios use` and `bios resume` return 1
  on an unknown flavour or nothing to resume to).
- Panics are caught at the top level (`main.rs`); the default panic hook is
  suppressed outside `SPARKLEBIOS_DEBUG=1`, and the raw-mode guard restores
  the terminal as the stack unwinds.
- The hook is guarded by `command -v bios` so uninstalling the binary cannot
  break `.zshrc`.
- No network dependency anywhere; CI enforces this with `cargo-deny`.

## 10. Testing

- **Golden files** (`tests/golden/*.txt`): `render_static`'s output for
  `pc95`, against `Facts::fixture()` (a frozen fact set), compared byte for
  byte, with and without findings, painted and unpainted.
- **Player parity**: `show::play` at speed 0 is asserted to produce the same
  final rows as `render_static`, so the animated and instant paths cannot
  drift apart.
- **Property tests**: every built-in flavour's quips and finding phrasings
  render within 80 columns and resolve against fixture facts; a shortened
  finding line always keeps its final sentence intact.
- **Unit tests per module**: the `BootMode` decision's boundaries (`mode.rs`),
  each check's probe logic against fake data (`checks/ports.rs`,
  `projects.rs`, `secrets.rs`), the findings cache's staleness and per-finding
  ttl rules (`cache.rs`), config parsing including the retired `full` and
  `fast` keys (`config.rs`), and the streak rules (`state.rs`).
- **Integration tests** (`tests/cli.rs`): the built binary, run end to end,
  across the whole command surface, including the exact `--help` text.
- **Doc parity** (`tests/themes.rs`): fails the build if a theme file and its
  entry in `docs/theme.md` ever disagree.
- **CI** (`.github/workflows/post.yml`, macOS and Linux): `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`, the em-dash and
  en-dash grep, and `cargo-deny` for licences, advisories and the no-network
  rule.

A shell-hook smoke test and a timing perf guard are not yet written.

## 11. Distribution

Repository: `reactivepixels/sparklebios`. Crate `sparklebios`, binary `bios`
(`neigh` once it exists). Local install is `cargo install --path .`.
Prebuilt binaries, a Homebrew tap, `cargo binstall` and a shell installer are
`ROADMAP.md` M8, not set up yet. macOS first; Linux fact probes are also M8.
Windows is out of scope. License: MIT OR Apache-2.0.

## 12. Milestones

Milestones and their definitions of done live in [`ROADMAP.md`](../ROADMAP.md),
kept there rather than duplicated here so the two documents cannot drift
apart. Each one ends in something usable daily.

## 13. Decisions taken

| # | Decision | Why | Alternative |
|---|---|---|---|
| D1 | Brand **SparkleBIOS**, crate `sparklebios`, binaries `bios` and `neigh` | `unicorn` is taken on crates.io and Homebrew and an unrelated `unicorn-bios` project exists; `sparklebios` and `bios` were free everywhere (checked 2026-09-19). UNICORN survives on screen as the fictional computer, and "Rainbows & Unicorns, Inc." as the fictional vendor | None, decided |
| D2 | Rust, single crate | Runs on every tab and every `cd`; a few ms startup vs 50ms or more for Node. Toolchain is already installed (1.96) | zsh script (tiny first version, but raw keys, caching and a TUI get ugly fast) |
| D3 | Sound only in the once-a-day full show | A terminal that beeps unprompted is a bad first impression | Always, or off |
| D4 | POST stays in scrollback | It doubles as a useful MOTD | Clear after the show |
| D5 | **Open source, public from the first usable milestone** (revised) | The goal is exposure and uptake; building in the open collects machine requests and testers early | Private until launch day |
| D6 | stable is parked; streak counts boot days for now | BIOS is the star; commit streaks arrive with the creature | Build the streak tracker early |
| D7 | zsh and macOS first, **Linux and bash/fish before the soft launch** (revised) | The screenshot-sharing audience is mostly on Linux | |
| D8 | **One screen, `pc95`, replaces the `pc85` and `c64` machines** (supersedes the original multi-machine plan in sections 3.1 and 3.5) | Not recorded: visible in the code and the changelog ("flavours carry the personality"), but the fuller reasoning was not written down | Keep multiple hardware-era screens, each with its own quips |
| D9 | Personality moved to **flavours**, a data file per personality applied on top of the one screen, rather than living in the screen file itself | Not recorded beyond the behaviour: a flavour supplies the mascot, wording and quips; the screen supplies the hardware | Keep personality on the machine file, one file per persona |
| D10 | A finding's phrasing is looked up in the current **flavour's** table first, falling back to the **screen's** own table | Lets a flavour inherit sensible default wording and override only the lines where its voice actually differs, rather than repeating every finding string (this is exactly what `unicorn`'s empty `[findings]` table does) | Require every flavour to write its own phrasing for every finding |
| D11 | **No probe ever runs on the boot path**, even on a cold cache; `bios refresh` is the only thing that probes, always detached and never waited on, and a cold cache simply shows nothing until the first refresh completes | So a slow or hanging probe can never slow down a shell starting, per principle 2 | Probe inline once on a cold cache, as the original design proposed, and let the memory count cover the wait |

## 14. Risks

- **Boot fatigue.** Mitigated by the mode rules, the burst window and (once
  built) the beige fail-safe. Needs a week of real use to tune.
- **Kitty graphics support** varies by embedding terminal; `graphics = auto`
  falls back to half-blocks anywhere it is not reported, and `blocks` opts
  out of it entirely for terminals that drop the image when a tab sleeps.
- **Keystroke handling** during the show remains the fiddliest part of the
  player. Disabling `ISIG` avoids a separate SIGINT handler, but has not been
  exercised against every terminal emulator.
- **Automated terminals.** Terminal managers and coding tools spawn many
  shells. Non-interactive ones are excluded by the hook itself; interactive
  bursts fall under the 10s quiet window.
- **Linux and other platforms.** `facts/other.rs` is a stub today: every fact
  is absent off macOS, so a screen there shows only what does not depend on a
  probe, until M8's Linux fact probes land.
