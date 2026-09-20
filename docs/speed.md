# Speed

The number in the README is not something measured once by hand. It is
measured on every push, by `tools/speed_receipt.py`, and CI reds the build if
either real case goes over budget.

## The three rows

The first boot of the day plays the animated Full show on purpose, seconds
long by design, so it is not a speed number worth tracking. The two rows
that are checked against a budget are the ones a person or a script actually
hits all day:

- **A new tab, boot already shown today.** Every ordinary new tab after the
  first one. `bios boot --hook` decides Fast (`src/mode.rs`) and draws the
  static screen at once, no animation.
- **A shell inside a script, boot skipped.** Every subshell, and every
  non-interactive script that has `SPARKLEBIOS_BOOTED` in its environment
  already. `bios boot --hook` decides Off and does nothing at all. This is
  the floor almost everything else pays.

A third row, **Starting any program at all, for comparison**, runs the exact
same harness against `/usr/bin/true`. It is not checked against a budget: it
exists so a reader can see how much of the two rows above is bios deciding
something, and how much is just what starting any process costs on this
machine.

## Budget

From `docs/design.md` section 9's "Budgets and safety rules" table, principle
2: never slow or break the shell, and every feature gets a time budget. The
"work before first frame" row there gives 30ms, and both real rows are held
to that single number, because both are timed the same way: the whole
process, wall clock, start to exit. The table's other figure (5ms, "off or
quiet decision") is measured from inside the process; this script only ever
sees a process from the outside and cannot reproduce that number honestly, so
it does not try to check a different quantity against it.

Either median over budget fails the build.

## How it runs the real hook

`bios boot --hook` opens `/dev/tty` directly (`src/boot.rs`) rather than
reading stdout, so a plain pipe or `/dev/null` is not enough: the process
would never reach the code being measured. The script runs it against a real
pty instead (Python's standard library `pty` and `subprocess` modules).

Each run gets its own freshly allocated pty, allocated and closed outside the
timed span, rather than one pty shared across every run. Sharing a single pty
across a series of separate child processes, each claiming it as their
controlling terminal, was tried and is unreliable: on macOS the second child
onward fails to claim it, consistent with the terminal being revoked once the
first child's session ends. A fresh pty per run avoids that, while still
keeping the allocation itself out of the number being measured: a real shell
already has a terminal, and only pays for spawning bios.

## Before timing: proving the cases are real

A receipt that measures nothing is worse than no receipt. Before any timing
loop runs, the script checks the two real cases once:

- The Fast case must actually draw a screen: at least 100 bytes, including
  text that only appears on the rendered boot screen.
- The Off case must write nothing at all: exactly zero bytes.

If either check fails, the script exits non-zero with a message naming which
case is not doing what its label claims, before a single timed run happens.

## Running it yourself

```
cargo build --release
python3 tools/speed_receipt.py target/release/bios
```

It builds its own sandbox (its own `HOME` and all four XDG directories), so
it never touches your real config, state or cache. It seeds a warm findings
cache directly rather than waiting on a real `bios refresh`: the sandboxed
`HOME` has no repositories, open ports or secrets to probe anyway, so a real
refresh would find nothing more than this does.

Each row runs 50 times, with the first 5 discarded as warm-up, and reports
the median and the slowest of the remaining 45. Pass `--summary <path>` to
append the table to a file (what CI does, into `$GITHUB_STEP_SUMMARY`)
instead of printing it. Pass `--runner <name>` to label the table.
