# Setup

`bios setup` is the CMOS Setup Utility: one full screen for every option,
instead of hand-editing `config.toml`.

## What it edits

The same file `bios config edit` opens: `~/.config/sparklebios/config.toml`
(or under `$XDG_CONFIG_HOME` if that is set). Saving writes through the same
hand-edit functions `bios use` and `bios sprinkles` already use, so comments,
blank lines and key order in a file you have hand-edited survive. Nothing is
written until you save.

## The rows

| Row | Cycles through | Config key |
|---|---|---|
| Flavour | Every flavour by name | `flavour` |
| Theme | Unchanged, then every Ghostty theme by name | Switches Ghostty's theme on save; not a `config.toml` key |
| Mascot | Shown, Hidden | `graphics` (`auto` or `off`) |
| Sprinkles | Off, Light, Full | `sprinkles` |
| Daily Show | Enabled, Disabled | `animate` |
| Boot Screen | Enabled, Disabled | `boot` |
| Turbo | On, Off | None. It does nothing. It never did |

Theme starts on Unchanged rather than assuming the terminal's active theme,
since that is not something `bios setup` can read back.

## Keys

Left and right cycle a row's value; up and down move between rows. Enter
previews the boot screen with every pending choice applied, without saving
anything, then waits for a key to return to the screen. F10 saves. Esc
leaves.

## Dialogs

F10 opens a save dialog, Y or Enter by default. Esc opens a discard dialog,
but only when something has actually changed since the screen opened; on a
clean screen Esc leaves at once. The discard dialog defaults to N, so Enter
on it returns you to the screen rather than losing your changes.

## `NO_COLOR` behaviour

`NO_COLOR` set to anything non-empty drops every escape sequence: no colour,
no reverse video for the selected row (a leading `>` marks it instead), just
the plain layout. The same rule `bios boot` and `bios fetch` already follow.
Without it, the screen draws in truecolor on a terminal known to render it
correctly, and in plain ANSI everywhere else.

## The 80x24 minimum

`bios setup` needs a terminal of at least 80 columns by 24 rows and checks
this before it draws anything. A smaller terminal gets one line on stderr
naming the size it found, and exits 1 rather than drawing a screen it cannot
fit.

## Filling the terminal

Unlike the boot screen, which sits transparently on your theme by default,
`bios setup` fills the whole terminal in a fixed CGA blue, the one it would
have used in 1995, regardless of which Ghostty theme or variant you run.
