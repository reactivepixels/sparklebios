# Security

## The short version

SparkleBIOS runs inside your shell startup, so we treat "it did something you
did not ask for" as a security problem, not a bug.

Report privately here:
**https://github.com/reactivepixels/sparklebios/security/advisories/new**

Please do not open a public issue for anything on this page. You will get a
reply within seven days.

## What counts

- Anything that lets a file in a repository run a command without the owner's consent. Project checks are ignored until a directory is allowed with `bios trust`; a way around that is a vulnerability.
- Anything that makes a boot, a project POST or the shutdown screen hang, crash a shell, or corrupt a terminal.
- Anything that writes outside the documented paths (`~/.config/sparklebios`, `~/.local/state/sparklebios`, `~/.cache/sparklebios`, and the Ghostty themes directory when you run `bios theme install`).
- Escape sequence injection: a way for text from the environment, a file name or a machine file to take control of the terminal.
- Any network access at all. There is no network code in this project, the build enforces it (`deny.toml`), and finding some would be the most serious report we could receive.

## What does not

- A machine file you wrote and installed yourself printing something rude. That one is on you.
- The unicorn. The unicorn is working as intended.

## Supported versions

Until 1.0, only the latest release gets fixes.
