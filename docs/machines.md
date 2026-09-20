# The screen

SparkleBIOS ships one screen, `pc95`. A [flavour](flavours.md) supplies its
personality: the mascot, the firmware and vendor wording, the streak line,
the footer code and the quips. The rest of this page documents the screen
file format, for anyone who wants to write their own.

**Read [docs/voice.md](voice.md) before writing a single word that a screen
will print.** Every line a screen shows is reviewed like code, against that
page's rules. Wording is never generated at runtime and never left to whoever
happens to be implementing the feature.

## Schema

Top level keys:

| Key | Type | Required | Notes |
|---|---|---|---|
| `id` | string | yes | Matches `[a-z0-9_-]+`. Names an era, for example `pc95`. No real logos, wordmarks or copied copyright strings |
| `name` | string | yes | A human readable label for the screen |
| `cols` | integer | yes | Terminal column width the screen is authored for. Must be between 20 and 200 |
| `fg` | string | yes | Normal text colour, `#RRGGBB`, either case |
| `bright` | string | yes | Bright text colour, `#RRGGBB` |
| `accent` | string | yes | Accent colour, `#RRGGBB` |
| `bg` | string | no | Background colour, `#RRGGBB`. When absent on a painted screen, the screen paints no background: see below |
| `paint` | boolean | no | Defaults to false. Paints the screen when the terminal is TrueColor and wide enough. Fills the block with `bg` when `bg` is set, or leaves it transparent when it is not |
| `border` | string | no | Colour, `#RRGGBB`. Draws a frame around a painted screen. Requires `paint` to be true and `bg` to be set |
| `pad_x` | integer | no | Defaults to 2. Blank painted columns left and right of the text, 0 to 8 |
| `pad_y` | integer | no | Defaults to 1. Blank painted rows above and below the text, 0 to 4 |
| `uppercase` | boolean | no | Defaults to false. Renders every line in upper case, after slot substitution |
| `logo` | string | no | Only `"unicorn"` exists today. Draws the mark at the top left of a painted screen |
| `badge` | array of strings | no | Up to 4 lines, each at most 20 characters, shown top right of a painted screen |
| `quips` | array of strings | no | Defaults to empty. One-line jokes, drawn from at boot |
| `flavoured` | boolean | no | Defaults to false. When true, the screen takes its quips and its logo sprite from the current flavour instead of its own `quips` and `logo`. See [flavours.md](flavours.md) |
| `detect_width` | integer | no | 4 to 60. When set, every `detect` step's label is rendered for slots and right-padded with spaces to this width (never truncated), instead of being used exactly as written |
| `[findings]` | table | no | Maps a finding id to its phrasing on this screen. Values may contain `{slot}`s, filled in from that finding's own facts, under the same omission rule as any other slot. See [checks.md](checks.md) for what a finding is and the ids that exist |
| `[[step]]` | array of tables | yes, at least one | The ordered screen content |

A step table has exactly one of six kinds: `print`, `count`, `detect`,
`quip`, `findings`, `f1`. Mixing two kinds, or a `quip`, `findings` or `f1`
step whose value is `false`, is invalid. A `quip` step is valid when the
screen has its own `quips`, or when `flavoured` is true (in which case the
quips come from the flavour at boot, and the screen needs none of its own).

| Kind | Fields | Defaults | Behaviour |
|---|---|---|---|
| `print` | `print` (text), optional `style`, optional `ms` | `ms` 45 | Prints one line of text |
| `count` | `count` (template), `to` (required), optional `suffix`, optional `ms` | `ms` 600, `suffix` empty | Prints a template line with `{n}` replaced by the resolved value of `to`, then the suffix appended |
| `detect` | `detect` (label), `result` (required), optional `style`, optional `ms` | `ms` 190 | Prints a label followed by a resolved result |
| `quip` | `quip = true`, optional `style`, optional `ms` | `ms` 120 | Prints one line drawn from the screen's `quips`, or from the current flavour's quips when `flavoured` is true |
| `findings` | `findings = true`, optional `ms` | `ms` 90 | Prints one line per current finding, in finding order, each phrased from `[findings]`. See [checks.md](checks.md) |
| `f1` | `f1 = true`, optional `ms` | `ms` 45 | Prints the F1 line, phrased from the `f1_resume` or `f1` key of `[findings]`. See [checks.md](checks.md) |

`style` is one of `"normal"`, `"bright"`, `"accent"` (TOML strings), and is
allowed on `print`, `detect` and `quip` steps. On a `detect` step it colours
only the result, never the label. `ms` is allowed on every step kind and
governs the timing of the animated boot; it is never part of the printed
text.

Text fields may contain `{key}` slots, filled in from the current facts (see
below). A slot uses one or more of `a-z`, `0-9`, `_`, `.`.

## The omitted-line rule

A line whose `{slot}` cannot be resolved is never printed with a placeholder
and never printed with an invented fact. The whole line is omitted instead.
This applies to `print`, `count` and `detect` steps, and to each candidate
quip in turn.

## How quips rotate

A boot picks a starting index into the screen's `quips` array and tries each
quip in order from there, wrapping around, printing the first one whose slots
all resolve. If none resolves, the `quip` step is omitted like any other
line. Across boots the starting index changes, so the same person sees a
different quip on a later boot rather than the same one every time. A quip
whose rendered text is longer than `cols` is treated the same as one whose
slots do not resolve: it is skipped in favour of the next candidate.

## Painted screens

When `paint` is true, the terminal is TrueColor, and it is wide enough for
the block, the screen is drawn as a solid `bg` rectangle with the text lines
inside it, `pad_x` and `pad_y` of blank border around them, and an optional
frame in the `border` colour. Any line's text is truncated to `cols`
characters so it always fits the block. When the terminal is not TrueColor,
is too narrow, or its width is unknown, `paint` is ignored and the screen
falls back to plain painted-free text. A `logo` and a `badge` only ever
appear on a painted screen at least 60 columns wide: the logo is drawn as
half-block characters or, on a terminal that supports it, as an actual
image, and the badge is right-aligned in the accent colour, its first line
on the second text row (never the first, so the header line is never cut
short) and one line per row after that. A row's own text is never
truncated to make room for a badge; if it would end within 2 cells of
where the badge starts, that row simply shows no badge.

A screen with `paint = true` and no `bg` paints transparently: the same
padding, logo placement and badge placement apply, but no background colour
is emitted anywhere, including in the logo. Text is drawn in its own
foreground colour only, and any transparent pixel in the logo shows the
terminal's own background. A `border` has nothing to frame on a transparent
screen, so it requires `bg` to be set.

## Facts available today

| Key | What it is | Example |
|---|---|---|
| `cpu.name` | Processor name, as the system reports it | `Apple M4 Pro` |
| `cpu.cores` | Logical core count | `14` |
| `mem.kb` | Installed memory in kilobytes, the unit a memory count deserves | `37748736` |
| `disk.size_gb` | Size of the volume holding your home directory, in decimal GB | `994` |
| `disk.free_gb` | Free space on that volume | `212` |
| `disk.used_pct` | How full it is, as a whole number | `78` |
| `os.name` | Operating system | `macOS` |
| `os.version` | Its version | `26.5` |
| `host.name` | Host name, without a trailing `.local` | `unicorn` |
| `shell.name` | The shell that is booting | `zsh` |
| `shell.boot_ms` | How long the shell took to start, measured. Absent when the shell has been running for more than 10 seconds | `412` |
| `date.bios` | Today, the way a BIOS writes it | `09/19/2026` |
| `date.year` | The year | `2026` |
| `date.today` | Today in ISO form | `2026-09-19` |
| `streak.days` | Consecutive days with a boot. Absent in previews | `12` |
| `streak.label` | The same, ready to print | `12 days` |

A fact that has not been gathered on a given boot is simply absent, and any
line referencing it is omitted under the rule above.

## User screens

The built-in screen lives in this repository's `machines/` directory and is
compiled into the `bios` binary. A user may add their own by placing a file
at `~/.config/sparklebios/machines/<id>.toml` (or under
`$XDG_CONFIG_HOME/sparklebios/machines` if that variable is set). The file
name must match the `id` inside it. A user file whose id matches `pc95`
replaces it; other user files add to the roster. A file that fails to parse
or fails validation is ignored, not reported, because nothing on the boot
path is allowed to complain.

## Trying a screen

```
bios boot --machine pc95            # preview one, without touching any state
NO_COLOR=1 bios boot --machine pc95 # the same, as plain text for pasting into an issue
```

A preview never reads or writes the streak, so the streak line is absent.

## Originality

Per [CONTRIBUTING.md](../CONTRIBUTING.md): original art and wording only. No
real logos, wordmarks or copied copyright strings. A screen's id names an era,
for example `pc95`, and real hardware is referenced descriptively, in the
style of, never copied verbatim.
