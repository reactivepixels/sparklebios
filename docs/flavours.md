# Flavours

A flavour is one TOML file: a personality. Where a machine is the hardware, a
flavour is the mascot, the firmware and vendor wording, one signature
"Detecting" line, the streak wording, the footer code and the quips. A
flavoured machine (`flavoured = true`, see [machines.md](machines.md)) draws
these from the current flavour instead of hardcoding them; `pc95` is the only
built-in machine that does. SparkleBIOS ships two flavours, `unicorn` (the
default) and `sumo`, and can load more from disk.

**Read [docs/voice.md](voice.md) before writing a single word that a flavour
will print.** Every line a screen shows is reviewed like code, against that
page's rules. Wording is never generated at runtime and never left to
whoever happens to be implementing the feature.

## Schema

All keys are required:

| Key | Type | Notes |
|---|---|---|
| `id` | string | Matches `[a-z0-9_-]+` |
| `name` | string | A human readable label, shown by `bios flavours` |
| `sprite` | string | Names a built-in sprite: a square transparent PNG and its 14 by 14 half-block grid file (see below) |
| `firmware` | string | The BIOS/firmware banner line |
| `vendor` | string | The copyright line |
| `board` | string | The board or chassis revision line |
| `cpu_gag` | string | The trailing joke on the processor line |
| `part` | string | The name of the one detected part (the unicorn flavour's is `Horn`) |
| `part_result` | string | The result shown for that part's `detect` line |
| `streak` | string | The boot streak line |
| `footer` | string | The trailing segment of the footer serial |
| `quips` | array of strings | At least 3 one-line jokes, in the same voice as a machine's own `quips` |

One more table is optional:

| Key | Type | Notes |
|---|---|---|
| `[findings]` | table | Overrides the machine's own `[findings]` phrasing, key by key. See [checks.md](checks.md) for what a finding is and the ids that exist |

## Findings

A flavour's `[findings]` table has the same shape as a machine's own (see
[machines.md](machines.md)): a map of finding id to phrasing, with the same
`{slot}` substitution and the same omission rule when a slot does not
resolve. When the current machine is flavoured, a finding id is looked up in
the flavour's table first; a key the flavour does not carry falls back to the
machine's own table for that id. The `unicorn` flavour deliberately has no
`[findings]` table at all, so it inherits the machine's phrasing unchanged.

## Slots inside a flavour's own strings

A flavour's strings may contain `{key}` slots, resolved against the facts
present at the moment the flavour is applied (see below), the same slot
syntax and the same omission rule a machine uses (see
[machines.md](machines.md)). The unicorn flavour's `vendor` uses
`{date.year}`; its `streak` uses `{streak.label}`; the sumo flavour's
`streak` uses `{streak.days}` instead. A quip may use any fact, the same as a
machine's own quips.

## The slots a flavoured machine can use

A flavoured machine's steps reference the flavour through these slots, filled
in only once the flavour has been applied:

| Slot | Comes from |
|---|---|
| `{flavour.firmware}` | `firmware` |
| `{flavour.vendor}` | `vendor` |
| `{flavour.board}` | `board` |
| `{flavour.cpu_gag}` | `cpu_gag` |
| `{flavour.part}` | `part` |
| `{flavour.part_result}` | `part_result` |
| `{flavour.streak}` | `streak` |
| `{flavour.footer}` | `footer` |

## How `apply` resolves a flavour

`flavour::apply` inserts `flavour.firmware`, `flavour.vendor`,
`flavour.board`, `flavour.cpu_gag`, `flavour.part`, `flavour.part_result`,
`flavour.streak` and `flavour.footer` into the current facts. Each value is
first rendered against the facts already present (so `{date.year}` inside
`vendor`, or `{streak.label}` inside `streak`, is resolved at this point, not
later). A value whose slots do not all resolve is not inserted at all, so any
machine line that references it is omitted under the usual rule, the same as
a preview never showing the streak line. `apply` never invents a value and
never leaves a raw `{slot}` in the output.

## Sprites

A flavour names a built-in sprite by its `sprite` key. Every sprite ships as
a pair of files under `sprites/`:

- `<name>.png`: a square, transparent PNG, shown as an actual image on a
  terminal with Kitty graphics support.
- `<name>14.txt`: the same artwork as a 14 by 14 half-block grid, for
  terminals without it. The file is self-describing: a palette of
  `X=#RRGGBB` lines (one character key each), a blank line, then 14 rows of
  14 characters. `.` is always transparent.

A flavour's `sprite` must name a sprite that exists; there is no separate
list to keep in sync.

## User flavours

Built-in flavours live in this repository's `flavours/` directory and are
compiled into the `bios` binary. A user may add their own by placing a file
at `~/.config/sparklebios/flavours/<id>.toml` (or under
`$XDG_CONFIG_HOME/sparklebios/flavours` if that variable is set). The file
name must match the `id` inside it. A user file whose id matches a built-in
replaces it; other user files add to the roster. A file that fails to parse
or fails validation is ignored, not reported, because nothing on the boot
path is allowed to complain.

## Commands

```
bios flavours                 the roster
bios use <flavour>            set the flavour
bios boot --flavour <id>      preview the screen with that flavour, without touching any state
```

A real boot's flavour comes from `flavour` in
`~/.config/sparklebios/config.toml`, the same file `bios use` writes. It
defaults to `unicorn`; an unknown id falls back to `unicorn` rather than
failing the boot.
