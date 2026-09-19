<p align="center">
  <img src="docs/assets/banner.svg" alt="SparkleBIOS" width="860">
</p>

<p align="center">
  <strong>A 1995 POST screen for your terminal that is secretly a health check.</strong>
</p>

<p align="center">
  <a href="https://github.com/reactivepixels/sparklebios/actions/workflows/post.yml"><img src="https://github.com/reactivepixels/sparklebios/actions/workflows/post.yml/badge.svg" alt="POST status"></a>
  <img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-1D9BE3?style=flat-square" alt="License: MIT OR Apache-2.0">
  <img src="https://img.shields.io/badge/network%20calls-0-62BB47?style=flat-square" alt="Network calls: 0">
  <img src="https://img.shields.io/badge/horns-1-FCB827?style=flat-square" alt="Horns: 1">
  <img src="https://img.shields.io/badge/bands-7-BB62C0?style=flat-square" alt="Bands: 7">
  <img src="https://img.shields.io/badge/est.-1985-E0453F?style=flat-square" alt="Established 1985">
</p>

# SparkleBIOS

```
     Sparkle Modular BIOS v1.985PG, An Enchantment Star Ally
     Copyright (C) 1985-2026, Rainbows & Unicorns, Inc.

UNI-440BX ACPI BIOS Revision 1007

Main Processor : Apple Silicon, 14 cores, 0 horns
Memory Testing : 37748736K OK

Detecting Primary Master   ... APPLE SSD AP1024Z
Detecting Primary Slave    ... node v22.23.0
Detecting Secondary Master ... cargo, swift, python3
Detecting Horn             ... 1 found (7-band, sparkle capable)

Primary Master S.M.A.R.T. status BAD, 94% full. Backup and replace.
Press F1 to continue

Boot time: 412ms
```

> **Status: pre-alpha.** Milestones M0 to M2 work from source: the theme, the hook, three painted boot machines, the unicorn, and the animated show. The health checks come next, and the screen above previews where they are headed.

## Install from source

```
cargo install --git https://github.com/reactivepixels/sparklebios
```

Then add this line to the END of your `~/.zshrc`:

```
command -v bios >/dev/null 2>&1 && eval "$(bios init zsh)"
```

Open a new tab. It boots.

For the theme: `bios theme install`, then follow the four lines it prints.

To try it without touching your shell: `bios boot --full`

## What it is

Every new terminal tab boots. The boot screen is period correct, takes about
300ms, and every line on it is true: that is your real disk, those are your real
runtimes, and when it says your drive is failing it means the disk is 94% full.

SparkleBIOS is three things that share one binary and one visual language:

| Part | What it does |
|---|---|
| **The BIOS** | A POST screen on every new tab that is really a health check. Several boot machines, from a mid-90s PC to an 8-bit home computer. `bios setup` is a blue BIOS setup utility, because of course it is. |
| **neigh** | A rainbow pipe with era palettes: `make \| neigh`. Bands, dither and raster bars, never gradients, because 1985 had sixteen colours on a good day. |
| **The theme** | "Rainbows and Unicorns", a serious 1985 Ghostty theme. The name is the only joke in it. |

## The jokes are true

| The screen says | It means |
|---|---|
| Primary Master S.M.A.R.T. status BAD | Your disk is over 90% full |
| CMOS checksum error, defaults loaded | Your `.zshrc` changed since the last boot |
| NVRAM: 3 stashes older than 30 days | You have git stashes you forgot about |
| Boot device slow: 1204ms | Your shell startup is slow, and that number is measured |
| CMOS battery low | Your laptop battery is low |
| Detecting backend ... NOT CONNECTED | This repo has no backend credentials of its own |

## Principles

1. **Be cool and hilarious.** That is the mandate. The rulebook is [docs/voice.md](docs/voice.md).
2. **Never slow or break the shell.** Under 30ms of work before the first frame. A failure prints nothing and exits 0. One environment variable turns it all off.
3. **Jokes carry facts.** Every gag line is backed by a real probe.
4. **Quantised, not gradient.** Three to sixteen colours and hard edges.
5. **Each machine speaks in its own voice.** The same warning reads differently on a PC, a VMS login and a home computer.
6. **Content is data.** Boot machines, palettes and gags are files, not code.
7. **No network.** There is no network code in this project and there never will be.

## Roadmap

| # | Milestone | Status |
|---|---|---|
| M0 | The theme: four Ghostty theme files | done |
| M1 | BIOS skeleton: shell hook, boot modes, static POST screens | done |
| M2 | The look and the show: painted screens, the unicorn, the `c64` machine, animation, skip keys, typeahead preserved | done |
| M3 | Health checks with machine-voiced warnings | planned |
| M4 | neigh | planned |
| M5 | `bios setup` | planned |
| M6 | Project POST when you `cd` into a repo | planned |
| M7 | More boot machines | planned |
| M8 | Launch readiness: Linux, bash and fish, `bios fetch`, prebuilt binaries | planned |

Details live in [ROADMAP.md](ROADMAP.md), and the rest of the paperwork is in [the manual](docs/README.md).

## Add your childhood computer

A boot machine is a single TOML file: some geometry, some colours, an ordered
list of steps, and how that machine phrases bad news. If the computer you grew
up with is not on the list, it should be. See [CONTRIBUTING.md](CONTRIBUTING.md).
The format is documented in [docs/machines.md](docs/machines.md). If you would
rather not write TOML, open an issue and tell us which machine and what its boot
screen said.

## Questions people ask

**Will it slow my shell?** It has a time budget and the budget is tested. Measured
on an Apple M3 Pro: 2.5ms for a whole boot, process start included. It spawns
nothing and waits for nothing.

**Does it phone home?** No. See principle 7. In 1985 there was nothing to phone.

**How do I turn it off?** `SPARKLEBIOS_BOOT=0`, or remove one line from your
shell config. The hook is guarded, so uninstalling the binary cannot break
your shell either.

**Can I skip the show?** Press any key. Whatever you typed is waiting on your prompt when it ends.

## System requirements

- A terminal.
- 640K of RAM (ought to be enough for anybody).
- One (1) unicorn. Supplied.

Warranty void if horn is removed. Horn is not hot-swappable.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Known issues: none. Known unicorns: 1.
