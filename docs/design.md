# SparkleBIOS: project design

Date: 2026-09-19
Status: accepted

## 1. What this is

A retro terminal toolkit with a 1985 sensibility. Serious engineering under an
unserious surface. It has three parts that share one binary, one config and one
visual language:

| Part | What it does | Role |
|---|---|---|
| **The BIOS** | Boots every new terminal tab with a period-correct POST screen that is secretly a health check | The star |
| **neigh** | Rainbow pipe with quantised era palettes (`make \| neigh`) | The paint |
| **The theme** | "Rainbows and Unicorns", a serious 1985 Ghostty theme, dark and light pair | The ground |

Parked for a later phase: **stable** (the unicorn creature in the prompt, commit
streaks, the heckler) and exit code theatre. The architecture leaves room for
them but nothing in this design depends on them.

### Principles

1. **Be cool and hilarious.** That is the mandate. Deadpan delivery, period-correct detail, gags that reward people who remember the real thing. `docs/voice.md` is the rulebook. Everything below is how we get away with it.
2. **Never slow or break the shell.** Every feature has a time budget and a kill switch. A failure prints nothing and exits 0.
3. **Jokes carry facts.** Every gag line is backed by a real probe. "S.M.A.R.T. status BAD" means the disk really is nearly full.
4. **Quantised, not gradient.** Bands, dither and raster bars. Three to sixteen colours. No smooth truecolor blends.
5. **Each machine speaks in its own voice.** A mid-90s PC, a VMS login and an 8-bit home computer phrase the same warning differently.
6. **Content is data.** Machines, palettes and gags are files, not code, so adding one never needs a recompile of logic.
7. **The name is the only joke in the theme.** The theme itself is a daily driver with checked contrast.

## 2. Command surface

One Rust binary, `bios`, plus `neigh` as a multi-call name (symlink to the
same binary, dispatched on `argv[0]`).

```
bios init zsh          print the shell hook (eval it at the END of .zshrc)
bios boot              run the POST for a new shell (called by the hook)
bios post              project mini POST (called by the chpwd hook)
bios halt              shutdown screen (called by the zshexit hook)
bios setup             BIOS setup utility (the settings TUI)
bios refresh           refresh the slow-fact cache (spawned detached, never waited on)
bios theme list|install|use <name>
bios trust [dir]       allow a repo's custom project checks
bios machines          list boot machines; `bios boot --machine c64 --full` to preview one
bios machines new|lint scaffold and validate a contributed machine
bios fetch             static POST on demand, for screenshots (the neofetch slot)

neigh [-p PALETTE] [--bands|--diag|--dither|--raster|--flash] [-w N]
      [--gallop] [--crt] [--banner TEXT] [--rule] [--force-color]
```

Shell support: zsh first. bash and fish hooks later; nothing in the binary is zsh specific.

## 3. The BIOS

### 3.1 Boot modes

| Mode | When | Length | Machine |
|---|---|---|---|
| **Full show** | First boot of the day | about 2s, skippable | The configured flagship (default: pc95) |
| **Fast** | Every other new tab | about 300ms | pc85 (memory count, one banner, done) |
| **Quiet** | A boot happened under 10s ago (burst of tabs, Supacode spawning surfaces) | 0ms | Nothing, unless a check failed, then one line |
| **Off** | See below | 0ms | Nothing |

Off conditions, checked first and cheaply: shell not interactive, stdout not a
tty, `TERM=dumb`, `SPARKLEBIOS_BOOT=0`, `SPARKLEBIOS_BOOTED` already exported (nested
shell), or a user-configured skip rule matches (for example `TERM_PROGRAM=vscode`).

The final POST stays in scrollback like a MOTD. It is not cleared.

### 3.2 The timeline

A boot is compiled into a timeline, then played:

```
Step = Print(line) | Count{template, to, ms} | Detect{label, result, ms} | Pause(ms) | Beep(code)
```

- The **player** runs a timeline against the tty with a speed factor and watches for keys.
- Speed = infinity renders the final static frame. That one code path serves fast mode's end state, non-animated terminals, reduced theatrics, and snapshot tests.
- **Esc** (or any key) finishes instantly. **DEL** finishes and opens `bios setup`.
- **Typeahead is preserved.** Keys typed during the show are not lost: the binary draws on `/dev/tty`, returns swallowed keystrokes on stdout, and the hook pushes them into the line editor with `print -z`.
- Raw mode is held by a guard that restores the terminal on exit, panic and SIGINT.

### 3.3 Facts

Facts fill the template slots: `{cpu.name}`, `{cpu.cores}`, `{mem.kb}`,
`{disk.model}`, `{disk.used_pct}`, `{runtime.node}`, `{shell.boot_ms}`, `{streak.days}`.

- **Fast facts** are read inline: memory and CPU via sysctl, disk usage via statfs, shell start time via `proc_pidinfo` on the parent pid.
- **Slow facts** come from a cache and are never computed during a boot: disk model, runtime versions (keyed by binary path and mtime), battery, brew outdated count. `bios refresh` rebuilds the cache detached, at most hourly.
- **The memory count is the profiler.** `{shell.boot_ms}` is real: now minus the parent shell's start time, microsecond precision, no two-part hook needed. The last POST line reports it.
- First boot ever has an empty cache. The probes run inline once and the memory count covers the wait, which is what a memory count was always for.

### 3.4 Checks

A check is `{id, probe, ttl, severity, beep}` and yields a finding. v1 set:

| Check | Finding | pc95 phrasing |
|---|---|---|
| Disk over 90% full | Fail | Primary Master S.M.A.R.T. status BAD, 94% full. Backup and replace. |
| `~/.zshrc` or SparkleBIOS config changed since last boot | Info | CMOS checksum error, defaults loaded |
| Last repo has uncommitted changes | Warn | Floppy disk(s) fail (40): uncommitted changes in `<repo>` |
| Stashes older than 30 days | Warn | NVRAM: 3 stashes older than 30 days |
| Shell startup over 800ms | Warn | Boot device slow: 1204ms. Check .zshrc |
| Brew outdated over 25 | Info | 31 option ROMs out of date |
| Battery under 15% on battery power | Warn | CMOS battery low |
| System clock implausible | Fail | CMOS battery failed, date set to 01-01-1980 |

- Any Warn or Fail ends the POST with "Press F1 to continue". The line is theatre: it never waits for a key, because blocking the prompt would break principle 2.
- A Fail is shown even in quiet mode, as one line.
- Users add global checks in config: `name`, `cmd`, `expect`, `severity`, `ttl`.

### 3.5 Machines

A machine is a TOML file: geometry, colours, the ordered steps, a pool of rotating
one-line quips, and a phrasing table for findings with a generic fallback. Built-ins are embedded with
`include_str!` and can be overridden or extended from `~/.config/sparklebios/machines/`.

```toml
id = "pc95"
cols = 80
fg = "#AAAAAA"
bg = "#000000"

[[step]]
print = "     Sparkle Modular BIOS v1.985PG, An Enchantment Star Ally"
style = "bright"

[[step]]
count = "Memory Testing : {n}K"
to = "{mem.kb}"
ms = 600
suffix = " OK"

[[step]]
detect = "Detecting Primary Master  "
result = "{disk.model}"
ms = 190

[findings]
disk_full = "Primary Master S.M.A.R.T. status BAD, {disk.used_pct}% full. Backup and replace."
```

Machine ids name an era, and the real hardware is only referenced descriptively:
**pc95** (mid-90s POST in the Award style, the flagship) and **pc85** (1985 PC/AT
style, the fast mode) come first. Then `c64`, `vms`, `dos`, `zx` and `mac84` (an
original compact-Mac homage with a horn). "Era follows theme" is a config mapping:
Six Stripes boots pc95, EGA '85 boots pc85, Workbench boots a Kickstart-style
machine, Paper White boots mac84.

All pixel art and wording is original. No real logos or wordmarks ship in the product.

Pixel art (horn logo, the mac84 icon) renders as half-block characters everywhere.
Kitty graphics protocol is an enhancement where the terminal reports support.

### 3.6 Sound, shutdown, calendar

- **Beep codes** via `afplay` on bundled square-wave WAVs: one short = all clear, one long two short = dirty repo, three short = a Fail finding. Default: sound only in the once-a-day full show.
- **Shutdown**: the zshexit hook shows "It's now safe to turn off your unicorn." in orange on black, held 350ms, top-level interactive shells only.
- **Calendar gags** are data too: Friday the 13th virus scare, 01-01-1980, the project's own birthday (19 September).

## 4. Project POST

On `chpwd` into a repo root (not on every directory change, and at most once per
repo per 10 minutes), print a three to five line mini POST in the active machine's voice.

Zero-config auto-detected checks:

- `.nvmrc` or `engines.node` vs the active node version
- `.env.local` or `.env` present when an example file exists
- Docker running when a compose file exists
- **Backend binding**: if the repo uses Supabase (a `supabase/` dir or the client in `package.json`), verify this repo's own binding exists (`BACKEND.md`, or `.env.local` with `NEXT_PUBLIC_SUPABASE_URL` and keys). Missing: "Detecting backend ... NOT CONNECTED. Press F1 to authenticate this project's own account."

Custom checks live in an optional `.bios.toml` in the repo. Because they run
commands, they are ignored until the directory is allowed with `bios trust`
(the direnv model). Auto-detected checks only read files and never need trust.

Budget: 20ms. Anything slower must come from cache.

## 5. neigh

- Reads stdin, writes painted stdout, streams line by line.
- Existing SGR colour in the input is stripped (other escapes pass through); width comes from `unicode-width`.
- When stdout is not a tty it passes input through untouched unless `--force-color`.
- **Default palette is `ansi`**: it emits ANSI indices 2, 3, 9, 1, 5, 4, so the installed theme decides what the rainbow looks like. Named palettes emit truecolor: `six`, `cga`, `ega`, `c64`, `amber`, `sticker`, plus user palettes from config.
- Modes: `--bands` (one colour per line), `--diag` (default, stepped diagonal), `--dither` (2x2 ordered dither at band edges, per character), `--raster` (copper bars behind text, text flips between ink and paper), `--flash` (text untouched, four-stripe flash on the right edge).
- `--banner TEXT` draws six-row block letters, one stripe per row. `--rule` draws the dithered spectrum rule. `--gallop` animates while stdout is a tty and input is finite. `--crt` dims alternate rows with SGR faint (the real tube effect belongs to the Ghostty shader).
- The BIOS uses neigh's painter for banners and rules, so boot screens follow the active palette.

## 6. The theme

- Files: `rainbows-and-unicorns` (Six Stripes, the lead), `-paper` (light half of the pair), `-ega`, `-workbench`. Exact palettes and the reasoning behind them are in `docs/theme.md`; contrast was checked for all four.
- `bios theme install` copies them into the Ghostty user themes directory and prints the config lines. It never edits the Ghostty config unless passed `--write`.
- Optional extras, later: a CRT `custom-shader` (off by default) and a matching starship palette.
- **Verify first (M0):** apps that embed the Ghostty engine (Supacode, for example) bundle their own Ghostty resources and may not read user themes from the standard locations. M0 confirms where themes load from in stock Ghostty and in at least one embedder.

## 7. Setup utility

`bios setup` is a classic blue BIOS setup screen built with ratatui. Arrow keys,
Enter, Esc, F10 to save. It edits the same TOML file a person could edit by hand.

| Menu | Real settings |
|---|---|
| Standard Sparkle Setup | flagship machine, fast machine, era-follows-theme |
| Boot Theatrics | full/fast/quiet/off rules, burst window, skip rules, shutdown screen |
| Palette Configuration | theme variant, neigh default palette, mode, band width, gallop speed |
| POST Checks | enable, thresholds, custom checks |
| Project POST | on/off, cooldown, trusted directories |
| Integrated Horn Peripherals | sound: off, full show only, always |
| Load Fail-Safe Defaults (Beige) | safe mode: no animation, no sound, no colour, checks only |

## 8. Files on disk

| Path | Purpose |
|---|---|
| `~/.config/sparklebios/config.toml` | settings (the only file a person edits) |
| `~/.config/sparklebios/machines/`, `palettes/` | user overrides and additions |
| `~/.local/state/sparklebios/state.json` | last boot time, boot-day streak, zshrc checksum, per-repo cooldowns, trust list |
| `~/.cache/sparklebios/facts.json` | slow facts with timestamps |

## 9. Architecture

Single crate, library plus thin binary, module boundaries chosen so each can be
tested alone:

```
src/
  term/      tty open, capability detection (truecolor, kitty graphics), raw-mode guard, cell buffer
  palette/   colour maths, built-in palettes, user palette loading
  paint/     band functions (bands, diag, dither, raster, flash), banner font, rule   <- neigh's engine
  facts/     fast probes, cache read/write, refresh job
  checks/    check definitions, findings, user checks
  machine/   TOML schema, template slots, findings phrasing
  post/      timeline compiler (machine + facts + findings -> steps), player, boot-mode decision
  project/   repo detection, auto checks, .bios.toml, trust
  setup/     ratatui screens <-> config
  theme/     theme files, install
  shell/     hook templates (zsh)
  cli.rs     clap, multi-call dispatch
machines/  themes/  palettes/  sounds/      embedded data
```

Data flow for a boot: `decide mode -> load facts (fast + cache) -> run checks ->
compile timeline from machine -> play on /dev/tty -> write state -> spawn refresh detached`.

### Budgets and safety rules

| Path | Budget |
|---|---|
| Off or quiet decision | under 5ms |
| Work before first frame | under 30ms |
| Project POST | under 20ms |
| Fast show total | about 300ms |

- Boot, post and halt never return non-zero and never print errors unless `SPARKLEBIOS_DEBUG=1`.
- Panics are caught at the top level; the raw-mode guard always restores the terminal.
- The hook is guarded by `command -v bios` so uninstalling the binary cannot break `.zshrc`.
- No network access anywhere.

## 10. Testing

- **Snapshot tests** (insta): every machine rendered at speed = infinity against a fixed facts fixture, with and without findings.
- **Timeline tests**: step durations sum within each mode's budget; skip produces the same final frame as a full play.
- **Painter property test**: painted output with SGR stripped equals the input, for all modes and palettes.
- **Checks**: each check against fake probes; cache TTL and stale-while-revalidate behaviour.
- **Hook test**: `zsh -ic` smoke script asserting no output when non-interactive, typeahead pass-through, and the kill switch.
- **Perf guard**: a hyperfine script for the quiet path and first-frame budget.

## 11. Distribution

Repository: `reactivepixels/sparklebios`, public from the first commit. Crate and
Homebrew formula `sparklebios`; binaries `bios` and `neigh`. Local install is
`cargo install --path .`; releases use cargo-dist prebuilt binaries, a Homebrew tap,
`cargo binstall` and a shell installer. macOS first, Linux before the soft launch
(only the fact probes differ). Windows is out of scope. License: MIT OR Apache-2.0.

## 12. Milestones

Each milestone ends in something usable daily.

| # | Milestone | Done when |
|---|---|---|
| M0 | **Theme** | Four theme files install and load in this terminal (Supacode question answered); README shows the config lines |
| M1 | **BIOS skeleton** | `bios init zsh` hook, boot-mode decision, fast facts, pc95 and pc85 rendered statically, kill switch, budgets met |
| M2 | **The show** | Timeline player, skip and DEL, typeahead preserved, once-a-day full show, beeps, shutdown screen |
| M3 | **Health checks** | Check engine, fact cache and detached refresh, machine-voiced findings, F1 line, beep codes |
| M4 | **neigh** | All palettes and modes, banner, rule, gallop; BIOS banners painted by neigh |
| M5 | **Setup** | BIOS setup TUI edits config; DEL during boot opens it; beige fail-safe |
| M6 | **Project POST** | chpwd hook, auto checks including backend binding, `.bios.toml`, trust |
| M7 | **More machines** | c64, vms, dos, zx, mac84; era follows theme; calendar gags; kitty graphics where supported |
| M8 | **Launch readiness** | Linux fact probes; bash and fish hooks; `fetch`; machine scaffold, lint and CI preview rendering; cargo-dist binaries and Homebrew tap; README, VHS tapes, site with "boot your browser" |
| Later | stable, exit code theatre, starship preset, bash and fish hooks, Homebrew tap | |

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

## 14. Risks

- **Boot fatigue.** Mitigated by mode rules, the burst window, skip rules and the beige fail-safe. Needs a week of real use to tune.
- **Supacode theme loading and kitty graphics support** are both unverified inside the embedded Ghostty engine. M0 and M7 start by testing them.
- **Keystroke handling** during the show is the fiddliest part of M2. The fallback is no key handling at all in fast mode (300ms is short enough).
- **Automated terminals.** Terminal managers and coding tools spawn many shells. Non-interactive ones are already excluded; interactive bursts fall under the 10s quiet window.
