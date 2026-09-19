# Roadmap

Each milestone ends in something usable every day. The reasoning behind all of
this is in [docs/design.md](docs/design.md).

| # | Milestone | Done when |
|---|---|---|
| M0 | **Theme** | Five Ghostty theme files (`rainbows-and-unicorns`, `-paper`, `-ega`, `-workbench`, `-mane`) load in stock Ghostty; the README shows the config lines; where embedded Ghostty engines load themes from is documented |
| M1 | **BIOS skeleton** | `bios init zsh` hook, boot-mode decision, fast facts, `pc95` and `pc85` rendered statically, kill switch, time budgets met |
| M2 | **The look and the show** | Painted screens with borders, the unicorn logo and badge, the `c64` machine, the timeline player, any key to skip, typeahead preserved, once-a-day full show. Beep codes and the shutdown screen moved to M3 |
| M3 | **Health checks** | Check engine, fact cache with detached refresh, machine-voiced findings, the F1 line |
| M4 | **neigh** | All palettes and modes, banner, rule, gallop; BIOS banners painted by neigh |
| M5 | **Setup** | The BIOS setup TUI edits the config file; DEL during boot opens it; beige fail-safe defaults |
| M6 | **Project POST** | `chpwd` hook, automatic checks including backend binding, `.bios.toml`, `bios trust` |
| M7 | **More machines** | `c64`, `vms`, `dos`, `zx`, `mac84`; era follows theme; calendar gags; kitty graphics where supported |
| M8 | **Launch readiness** | Linux fact probes; bash and fish hooks; `bios fetch`; machine scaffold, lint and CI preview rendering; prebuilt binaries and a Homebrew tap; website |

## Later

- **stable**: a unicorn that lives in your prompt, with commit streaks and opinions about your commands
- Exit code theatre
- A matching starship palette
- An optional CRT shader for Ghostty

## Machine wishlist

Machines people have asked for or that obviously belong. Each one is a good
first contribution once the machine format lands in M1.

- Amiga Kickstart style
- BBC Micro style
- Apple II style
- Atari ST style
- MSX style
- Amstrad CPC style
- Sun OpenBoot style
- SGI PROM style
- NeXT style
- BeOS style
- Windows 95 style
- A Linux kernel boot
- A handheld console logo drop
