# Contributing

SparkleBIOS is at its first release. The most useful things right now:

1. **Tell us about a boot screen.** Open an issue with the machine, the year, and what the screen actually said, ideally with a photo or an emulator screenshot. Accuracy is the whole joke.
2. **Try the theme** and report how it looks in your terminal.
3. **Read [docs/design.md](docs/design.md)** and poke holes in it.

## Ground rules

- **Be cool and hilarious.** It is the mandate. Read [docs/voice.md](docs/voice.md) before writing any line a screen will show. Deadpan or nothing.
- **Never slow or break the shell.** Anything on the boot path has a time budget and a test that enforces it.
- **Jokes carry facts.** A gag line with no real probe behind it does not ship.
- **No network code.** Not for updates, not for telemetry, not for anything.
- **Original art and wording only.** No real logos, wordmarks or copied copyright strings. Machines are named by era (`pc95`) and real hardware is referenced descriptively ("in the style of").
- **No em dashes or en dashes** in prose, comments or commit messages. Use a comma, a colon, parentheses, or two sentences.

## Adding a flavour

Start with `bios flavour new <id>`. It writes a starter file with every key
already filled in and a comment above each one, so you have something working
before you have changed a single word. A flavour is one TOML file plus a
sprite: a square transparent PNG, nothing else. The schema is documented in
[docs/flavours.md](docs/flavours.md). Preview yours with
`bios boot --flavour <id>`, and paste the output into the pull request.

## Development

```
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

All four must pass before a pull request is reviewed.

## Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). The jokes
here punch at hardware, eras and hardware envy. Never at people.

## License

By contributing you agree that your contribution is dual licensed under
MIT OR Apache-2.0, the same as the project.
