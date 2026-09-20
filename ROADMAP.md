# Roadmap

Each milestone ends in something usable every day. The reasoning behind all of
this is in [docs/design.md](docs/design.md).

| # | Milestone | Done when |
|---|---|---|
| M0 | **Theme** | Ten Ghostty theme files (`rainbows-and-unicorns`, `-paper`, `-ega`, `-workbench`, `-mane`, `-miami`, `-arcade`, `-vhs`, `-den`, `-sorbet`) load in stock Ghostty; the README shows the config lines; where embedded Ghostty engines load themes from is documented |
| M1 | **BIOS skeleton** | `bios init zsh` hook, boot-mode decision, fast facts, `pc95` and `pc85` rendered statically, kill switch, time budgets met |
| M2 | **The look and the show** | Painted screens with borders, the unicorn logo and badge, the `c64` machine, the timeline player, any key to skip, typeahead preserved, once-a-day full show. Beep codes and the shutdown screen moved to M3.5 |
| M3 | **Health checks** | Check engine, fact cache with detached refresh, flavour-voiced findings, the F1 line. Headline checks: **boot device order** (your recent projects and the state you left them in, with `bios resume`), **IRQ conflicts** (ports held by forgotten processes), and the **virus scan** (secrets and keys that git is tracking) |
| M3.5 | **More health checks** | Disk trend, dotfiles changed, stale stashes, battery, runtime drift, beep codes and the shutdown screen |
| M4 | **Sprinkles** | The optional delight layer, off by default, because not everyone wants sprinkles. One dial (`off`, `light`, `full`): text effects during the animated show (a shimmer across the firmware name, a twinkle around the mascot, a stripe sweep on streak milestones), and sound (the POST beep, beep codes, a short jingle per flavour). Respects reduced motion, never flashes, costs nothing when off. Arrives with `bios config edit`, `bios config path` and `bios config reset`, a fully commented `config.toml`, and per-flavour overrides for power users |
| M4.5 | **neigh** | The rainbow pipe: all palettes and modes, banner, rule, gallop |
| M5 | **Setup** | The BIOS setup TUI edits the config file; DEL during boot opens it; beige fail-safe defaults |
| M6 | **Project POST** | `chpwd` hook, automatic checks including backend binding, `.bios.toml`, `bios trust` |
| M7 | **More flavours** | Neko and Lo-fi join Unicorn and Sumo; rare quips and calendar gags; community flavours |
| M8 | **Launch readiness** | Linux fact probes; bash and fish hooks; `bios fetch`; machine scaffold, lint and CI preview rendering; prebuilt binaries and a Homebrew tap; website |

## Later

- **stable**: a unicorn that lives in your prompt, with commit streaks and opinions about your commands
- Exit code theatre
- A matching starship palette
- An optional CRT shader for Ghostty

## Flavour wishlist

Open an issue with the creature or object, and the one part of it a BIOS
should detect.
