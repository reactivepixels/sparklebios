# Config

`~/.config/sparklebios/config.toml` is a single TOML file that changes how
`bios` behaves: which flavour boots, whether the show is animated, how the
mascot is drawn, whether the health checks run, and where they look for your
projects.

## Where the file lives

The directory follows the XDG base directory rule: `$XDG_CONFIG_HOME/sparklebios`
when `XDG_CONFIG_HOME` is set to a non-empty value, otherwise
`~/.config/sparklebios`. The file itself is `config.toml` inside that
directory. `bios config path` prints the exact path for the machine you are
on.

## Every key is optional

A missing key falls back to its default. A file that cannot be read, or that
fails to parse as TOML, is ignored rather than reported: nothing on the boot
path is allowed to complain. An unknown key, including a retired one, is
ignored too, so an old config file never breaks a new `bios`.

## Keys

| Key | Type | Default | Meaning |
|---|---|---|---|
| `flavour` | string | `"unicorn"` | Which personality boots. Run `bios flavours` for the roster. Also set with `bios use <flavour>` |
| `animate` | boolean | `true` | Whether the first boot of the day is animated. Every other boot is drawn instantly |
| `graphics` | string | `"auto"` | Whether the mascot image is shown: `"auto"` shows it where the terminal can draw images, `"off"` never shows it. The retired `"image"` and `"blocks"` values, and any unknown value, read as `"auto"` |
| `checks` | boolean | `true` | Whether the health checks run: the findings shown on the boot screen, and the background refresh that feeds them |
| `boot` | boolean | `true` | The master switch. `false` means a new tab prints nothing at all. `SPARKLEBIOS_BOOT=0` does the same for one shell, and still wins over this |
| `project_dirs` | array of strings | `[]` | Where to look for your git repositories. The boot screen lists the three touched most recently, and `bios resume` takes you to the first. An empty array means the built-in list: `~/Code`, `~/code`, `~/Projects`, `~/projects`, `~/src`, `~/dev`, `~/Developer`, `~/repos`, `~/work` and `~/git` |

## Subcommands

- `bios config path`: prints the absolute path of the config file, whether or
  not it exists.
- `bios config edit`: opens the config file in your editor, creating it from
  the commented defaults first if it does not exist yet. Uses `$VISUAL`, then
  `$EDITOR`, then `vi`. Exits 1 if the editor cannot be launched or exits
  non-zero itself.
- `bios config reset`: overwrites the config file with the commented
  defaults, without asking.

## The backup `config reset` leaves

`bios config reset` never destroys a hand-edited config with no way back. If
a config file already exists and its contents differ from the commented
defaults, its old contents are saved to `config.toml.bak` in the same
directory first, replacing any previous backup. A file already identical to
the defaults produces no backup. Both the config file and the backup are
written atomically: a temp file next to the target, then a rename, so a
crash mid-write never leaves a half-written file in place of either one.

## Environment overrides

A handful of environment variables override the config file, for one run:

| Variable | Overrides | Effect |
|---|---|---|
| `SPARKLEBIOS_BOOT` | Whether `bios boot` plays at all | Set to `0` to stop it booting in a given shell |
| `SPARKLEBIOS_ANIMATE` | `animate` | Set to `0` to skip the animated show, even on the first boot of the day |
| `SPARKLEBIOS_GRAPHICS` | `graphics` | Either `auto` or `off`, with the same tolerance for retired and unknown values |
| `SPARKLEBIOS_BOOTED` | Nothing in the config file | Set by the shell hook once a boot has happened, so a shell started inside another shell does not boot a second time. Any value at all stops the boot, so unset it rather than setting it to `0` |
| `SPARKLEBIOS_DEBUG` | Nothing in the config file | Set to `1` to print errors that are otherwise silent, since nothing on the boot path is allowed to complain by default |
