# Security

## The short version

SparkleBIOS runs inside your shell startup, so we treat "it did something you
did not ask for" as a security problem, not a bug.

Report privately here:
**https://github.com/reactivepixels/sparklebios/security/advisories/new**

Please do not open a public issue for anything on this page. You will get a
reply within seven days.

## What actually runs

Nothing here spawns a process from a repository, or on a schedule, or in the
background beyond what is documented next. What runs, and when:

- The shell hook, on every new interactive tab: reads facts inline, reads the
  findings cache, and draws the boot screen.
- `bios refresh`, spawned detached by that same boot when the cache is stale:
  the only thing that runs a probe. It never runs inline, and a boot never
  waits on it.
- Whatever you type by hand: `bios setup`, `bios fetch`, `bios theme use`,
  `bios flavour new`, and the rest of the command surface in
  [the manual](docs/README.md).

Nothing else runs. There is no hook that fires on `cd`, no scheduled task, and
nothing that reads a per-project config file.

## What counts

- Anything that runs a command, or reads a file, outside what is listed above.
- Anything that makes a boot hang, crash a shell, or corrupt a terminal.
- Anything that writes outside the documented paths (`~/.config/sparklebios`, `~/.local/state/sparklebios`, `~/.cache/sparklebios`, and the Ghostty themes directory when you run `bios theme install`).
- Escape sequence injection: a way for text from the environment, a file name or a machine file to take control of the terminal.
- Any network access at all. There is no network code in this project, the build enforces it (`deny.toml`), and finding some would be the most serious report we could receive.

## What does not

- A machine file you wrote and installed yourself printing something rude. That one is on you.
- The unicorn. The unicorn is working as intended.

## Supported versions

Until 1.0, only the latest release gets fixes.
