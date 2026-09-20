# The manual

Kept in a binder, on a shelf, next to the machine.

| Document | What it is |
|---|---|
| [design.md](design.md) | The accepted design: what SparkleBIOS is, how a boot works, the time budgets, the milestones and every decision taken along the way |
| [voice.md](voice.md) | The comedy rulebook. Required reading before writing any line a screen will show |
| [theme.md](theme.md) | The "Rainbows and Unicorns" Ghostty theme: ten variants, exact palettes, contrast figures and the reasoning |
| [machines.md](machines.md) | The screen: SparkleBIOS ships one, `pc95`, and this documents its TOML schema, the facts you can use, and how quips rotate, for anyone writing their own |
| [flavours.md](flavours.md) | How to write a flavour: the personality behind a flavoured machine, its sprite pair, and how it resolves slots |
| [checks.md](checks.md) | How the health checks work: what a check probes, the fact cache and its refresh, and how a finding gets its phrasing |
| [config.md](config.md) | `config.toml`: where it lives, every key and its default, the `config path`, `config edit` and `config reset` commands, and the environment overrides |
| [presence.md](presence.md) | How the flavour keeps talking after the boot screen: the tab title, a long command, a typo, goodbye, and how each is decided without spawning anything |
| [sprinkles.md](sprinkles.md) | The optional effects layer on the animated show: the dial, what each level draws, the never-flash promise, and how to turn it off |
| [speed.md](speed.md) | The speed receipt: what `tools/speed_receipt.py` measures on every push, the budget it checks against, and how to run it yourself |
| [setup.md](setup.md) | `bios setup`: what it edits, the rows, the keys, the dialogs, `NO_COLOR` behaviour and the 80x24 minimum |

Elsewhere in the repository:

| File | What it is |
|---|---|
| [../README.md](../README.md) | The front of the box |
| [../ROADMAP.md](../ROADMAP.md) | Milestones and the flavour wishlist |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | House rules and how to help |
| [../CHANGELOG.md](../CHANGELOG.md) | The service log |
| [../SECURITY.md](../SECURITY.md) | How to report something that could hurt a shell |
| [../CODE_OF_CONDUCT.md](../CODE_OF_CONDUCT.md) | The Contributor Covenant. The jokes punch at hardware, never at people |
