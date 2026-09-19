# Machines

A machine is one TOML file: a boot screen. It declares an identity, four
colours, an optional list of quips, and an ordered list of steps. SparkleBIOS
ships two built-in machines, `pc95` and `pc85`, and can load more from disk.

**Read [docs/voice.md](voice.md) before writing a single word that a machine
will print.** Every line a screen shows is reviewed like code, against that
page's rules. Wording is never generated at runtime and never left to whoever
happens to be implementing the feature.

## Schema

Top level keys:

| Key | Type | Required | Notes |
|---|---|---|---|
| `id` | string | yes | Matches `[a-z0-9_-]+`. Names an era, for example `pc95`, `pc85`. No real logos, wordmarks or copied copyright strings |
| `name` | string | yes | A human readable label, shown by `bios machines` |
| `cols` | integer | yes | Terminal column width the screen is authored for. Must be between 20 and 200 |
| `fg` | string | yes | Normal text colour, `#RRGGBB`, either case |
| `bright` | string | yes | Bright text colour, `#RRGGBB` |
| `accent` | string | yes | Accent colour, `#RRGGBB` |
| `bg` | string | no | Background colour, `#RRGGBB` |
| `quips` | array of strings | no | Defaults to empty. One-line jokes, drawn from at boot |
| `[[step]]` | array of tables | yes, at least one | The ordered screen content |

A step table has exactly one of four kinds: `print`, `count`, `detect`,
`quip`. Mixing two kinds, or a `quip` step whose value is `false`, is invalid.

| Kind | Fields | Defaults | Behaviour |
|---|---|---|---|
| `print` | `print` (text), optional `style`, optional `ms` | `ms` 45 | Prints one line of text |
| `count` | `count` (template), `to` (required), optional `suffix`, optional `ms` | `ms` 600, `suffix` empty | Prints a template line with `{n}` replaced by the resolved value of `to`, then the suffix appended |
| `detect` | `detect` (label), `result` (required), optional `style`, optional `ms` | `ms` 190 | Prints a label followed by a resolved result |
| `quip` | `quip = true`, optional `style`, optional `ms` | `ms` 120 | Prints one line drawn from the machine's `quips` |

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

A boot picks a starting index into the machine's `quips` array and tries each
quip in order from there, wrapping around, printing the first one whose slots
all resolve. If none resolves, the `quip` step is omitted like any other
line. Across boots the starting index changes, so the same person sees a
different quip on a later boot rather than the same one every time.

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
| `shell.boot_ms` | How long the shell took to start, measured. Absent when the shell has been running for more than a minute | `412` |
| `date.bios` | Today, the way a BIOS writes it | `09/19/2026` |
| `date.year` | The year | `2026` |
| `date.today` | Today in ISO form | `2026-09-19` |
| `streak.days` | Consecutive days with a boot. Absent in previews | `12` |
| `streak.label` | The same, ready to print | `12 days` |

A fact that has not been gathered on a given boot is simply absent, and any
line referencing it is omitted under the rule above.

## User machines

Built-in machines live in this repository's `machines/` directory and are
compiled into the `bios` binary. A user may add their own by placing a file
at `~/.config/sparklebios/machines/<id>.toml` (or under
`$XDG_CONFIG_HOME/sparklebios/machines` if that variable is set). The file
name must match the `id` inside it. A user file whose id matches a built-in
replaces it; other user files add to the roster. A file that fails to parse or
fails validation is ignored, not reported, because nothing on the boot path is
allowed to complain.

## Trying a machine

```
bios machines                    # the roster
bios boot --machine pc95         # preview one, without touching any state
NO_COLOR=1 bios boot --full      # the same, as plain text for pasting into an issue
```

A preview never reads or writes the streak, so the streak line is absent.

## Originality

Per [CONTRIBUTING.md](../CONTRIBUTING.md): original art and wording only. No
real logos, wordmarks or copied copyright strings. Machines are named by era
(`pc95`, `pc85`) and real hardware is referenced descriptively, in the style
of, never copied verbatim.
