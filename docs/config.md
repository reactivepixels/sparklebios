# Config

`~/.config/sparklebios/config.toml` is a single TOML file that changes how
`bios` behaves: which flavour boots, whether the show is animated, how the
mascot is drawn, whether the health checks run, where they look for your
projects, the sprinkles dial, and whether and how the flavour speaks away
from the boot screen.

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
| `sprinkles` | string | `"off"` | The sprinkles dial: `"off"`, `"light"` (text effects), `"full"` (text effects and sound) or `"ultra"` (the whole day). Also set with `bios sprinkles <level>`. See [sprinkles.md](sprinkles.md) |
| `ultra_chance` | integer (percent) | `20` | Ultra: how often an everyday command earns a reaction. Out of range (not 0 to 100) falls back to the default |
| `ultra_volume` | number | `0.5` | Ultra: sound volume, 0 to 1. `0` keeps the visuals and mutes the sound. Out of range (not 0 to 1) falls back to the default |
| `presence` | boolean | `true` | Whether the flavour speaks away from the boot screen: the finish line, the goodbye, the typo answer, and the in-character line from `refresh`, `resume` and `use`. `false` is silence. See [presence.md](presence.md) |
| `presence_after` | integer (seconds) | `10` | How long a command must run before the flavour comments on it |
| `title` | boolean | `true` | Whether the tab title carries the flavour's board name and the current folder |

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
| `SPARKLEBIOS_SPRINKLES` | `sprinkles` | `off`, `light`, `full` or `ultra`, for one run, with the same tolerance for an unknown value (it reads as `off`) |
| `SPARKLEBIOS_ULTRA_CHANCE` | `ultra_chance` | For one run, with the same tolerance for a value out of range or unparseable (it reads as the default) |
| `SPARKLEBIOS_ULTRA_VOLUME` | `ultra_volume` | For one run, with the same tolerance for a value out of range or unparseable (it reads as the default) |
| `SPARKLEBIOS_BOOTED` | Nothing in the config file | Set by the shell hook once a boot has happened, so a shell started inside another shell does not boot a second time. Any value at all stops the boot, so unset it rather than setting it to `0` |
| `SPARKLEBIOS_DEBUG` | Nothing in the config file | Set to `1` to print errors that are otherwise silent, since nothing on the boot path is allowed to complain by default |
| `SPARKLEBIOS_MASCOT` | Nothing in the config file | Set by the shell hook to the mascot's prompt placement, baked in once when the hook is generated. See [presence.md](presence.md) |
| `SPARKLEBIOS_PROMPT_IMG` | Nothing in the config file | For starship: set once your first prompt has drawn, so `bios prompt` sends only the placement from then on, not the image itself again. See [presence.md](presence.md) |
