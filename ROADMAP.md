# Roadmap

Each milestone ends in something usable every day. The reasoning behind all of
this is in [docs/design.md](docs/design.md).

| # | Milestone | Shipped |
|---|---|---|
| M0 | **Theme** | Ten Ghostty theme files (`rainbows-and-unicorns`, `-paper`, `-ega`, `-workbench`, `-mane`, `-miami`, `-arcade`, `-vhs`, `-den`, `-sorbet`), installed and switched with `bios theme install` and `bios theme use` |
| M1 | **BIOS skeleton** | The shell hook (`bios init`, for zsh, bash and fish), the boot mode decision, fast facts, the `pc95` screen rendered statically, the `SPARKLEBIOS_BOOT` kill switch, time budgets met |
| M2 | **The look and the show** | Painted screens with borders, the unicorn logo and badge, the timeline player, any key to skip, typeahead preserved, once-a-day full show |
| M3 | **Health checks** | All eight checks: boot device order, IRQ conflicts, the virus scan, disk trend, changed dotfiles, stale stashes, runtime drift and battery health, each cached and phrased by your flavour. `bios refresh` runs them again now |
| M4 | **Sprinkles** | The optional effects layer, off by default: `light` (text effects), `full` (the same, plus the POST beep and beep codes), and `ultra` (all of that plus a jingle per flavour, synthesised at runtime rather than shipped as files, folder remarks on `cd`, two git ceremonies, a shutdown click, the Ghostty scanline and cursor trail shaders, and `bios screensaver`). `bios sprinkles`, `bios config path`, `bios config edit` and `bios config reset` |
| M5 | **Setup** | `bios setup`, the CMOS Setup Utility: arrow keys, Enter to preview, F10 to save, Esc to leave, nothing written until you save |
| M6 | **Project POST** | Still to build: `bios trust`, and a full mini POST on entering a repo, running its own checks. Distinct from ultra's one line folder remark, which ships |
| M8 | **Launch readiness** | Still to build: the shutdown screen, prebuilt binaries and a Homebrew tap |

## Later, maybe

- **neigh**, the rainbow pipe
- **stable**: a unicorn that lives in your prompt, with commit streaks and opinions about your commands
- Exit code theatre
- **Horn Check**, a second game

## Flavour wishlist

Open an issue with the creature or object, and the one part of it a BIOS
should detect.
