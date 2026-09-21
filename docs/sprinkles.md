# Sprinkles

An optional effects layer on the once-a-day animated boot. Off by default,
because not everyone wants sprinkles.

## The dial

One key, `sprinkles`, four values:

| Value | What it does |
|---|---|
| `off` (default) | No sprinkle code runs. |
| `light` | Text effects: a shimmer, a twinkle, and (only on a streak milestone) a stripe sweep. |
| `full` | Everything `light` does, plus sound: the POST beep and the beep codes. |
| `ultra` | Everything `full` does, plus a whole day of it: a jingle at start, the finish beep, folder remarks on `cd`, a shutdown click, and a screensaver. See [ultra](#ultra) below. |

Set it with `bios sprinkles <off\|light\|full\|ultra>`, or run `bios
sprinkles` with no argument to print the level currently in effect.
`SPARKLEBIOS_SPRINKLES` overrides the config file for one run, the same way
`SPARKLEBIOS_GRAPHICS` overrides `graphics`.

`off`, `light` and `full` only ever run in the once-a-day animated show. A
fast, quiet, or off boot is untouched, byte for byte, whatever the dial is
set to. `ultra` is the exception: the boot show itself is unchanged from
`full`, but `ultra` keeps going in the shell for as long as the tab stays
open. See below.

## What `light` draws

- **Shimmer.** Once the firmware line finishes, a 3 cell highlight sweeps
  across it left to right, once, then the line returns to its normal style.
- **Twinkle.** While the memory count runs, up to three cells in the blank
  margin beside the mascot show a star, then a plus, then a full stop, then
  nothing, and nothing again after that. Only when a mascot is actually
  shown: with no mascot there is no margin to twinkle in, so it does not run
  at all.
- **Stripe sweep.** Only when the boot streak lands on a milestone (7, 30,
  100, 365, or a later multiple of 365), the streak line is drawn once,
  rainbow coloured, held briefly, then settles to its normal style.

Every one of these is skipped, not squeezed, on a terminal narrower than 60
columns, and stops at once the moment a key is typed: the show already
fast-forwards to its final screen on a key, and a sprinkle never outlives
that.

## What `full` adds

One BEL, written to the terminal once the memory count completes: the whole
of the default POST beep, and period correct. When the findings include a
`fail`, three BELs play before the findings print, 400ms apart.

The plan for this feature originally called for three beeps 180ms apart, the
same spacing a 1995 POST used for its beep codes. That figure is not used
here. Some terminals are configured to show BEL as a full screen flash
rather than play a sound, and three flashes in 540ms sits inside the range
associated with photosensitive seizures. 400ms keeps three short beeps
recognisable as a warning while staying under three flashes a second.

No audio files ship with SparkleBIOS, and nothing is played by spawning a
process: sound here is a single byte, written to the tty, nothing more.

## ultra

`ultra` is for someone who wants the whole day of it, not just the boot:
everything `full` does, plus sound and reactions that keep running for as
long as the shell tab stays open. Off unless you ask for it.

### Sound

Fifteen sounds ship with `ultra`, and none of them ship as files. `bios
sprinkles ultra`, and `bios init` whenever the effective level is already
`ultra`, synthesise them once into `$XDG_CACHE_HOME/sparklebios/sounds/` and
leave them there; a shell that already has every file pays for fifteen
`stat` calls to notice that, and generating a missing one from scratch takes
well under 200ms for all fifteen together.

Of the fifteen, only these still play on their own, and each one answers
something you just did rather than arriving out of nowhere:

- `post_ok`, `post_fail`: the finish line's own beep, good and bad, once a
  command has run long enough to earn one (see "A long command" below).
- `power_off`: the shutdown click, played once the shell exits.
- `jingle_<flavour>`: one small tune per built-in flavour (`unicorn`, `sumo`,
  `ninja`, `viking`, `luchador`, `yeti`, `raccoon`, `wizard`), played once a
  shell starts. A flavour you write yourself gets no jingle.

The other four are generated the same way and kept in the cache, but nothing
triggers them right now: `floppy_seek`, `hdd_chatter`, `hdd_spindown` and
`modem` all used to fire while you were in the middle of something else, at
random or on an everyday command like `cd` or `git status`, and that is
exactly the sort of unprompted noise that got pulled. Wiring one back to a
trigger is a small change to the shell hook, not a re-port, if it's ever
wanted again.

Playing one costs nothing running per prompt beyond starting the system's
own audio player, detached: `afplay` on macOS, or the first of `pw-play`,
`paplay` or `aplay` found on Linux. If none of those exist, nothing plays
and nothing complains either. The shell never waits on it.

A sixteenth sound, `memory_count`, is generated alongside the fifteen above
but is never reached through `_sparklebios_ultra_sound`: `bios` itself starts
it, timed to the memory count digits it is already drawing, on a real Full
daily boot, and on a hand typed `bios boot` too, since typing the command is
a request, the same reasoning that lets a hand typed `bios boot` ignore
`SPARKLEBIOS_BOOT=0`. Both still need `ultra` and still need a player, found
through `SPARKLEBIOS_ULTRA_PLAYER` exactly as everything else on this page,
so a terminal with no hook installed stays silent either way.

### Ghostty shaders

`ultra` also turns on two Ghostty shaders: scanlines, and a cursor trail.
Both `bios sprinkles ultra` and a `bios setup` save that lands on Ultra do
the same two things: write `scanlines.glsl` and `cursor-trail.glsl` into
`$XDG_CONFIG_HOME/sparklebios/shaders/`, then add a `custom-shader` line for
each into your Ghostty config. Ghostty only picks up a `custom-shader`
change on a reload, not a new tab, so reload Ghostty (or restart it) to see
them; a new tab is enough for the sound above, but not for this.

Dropping `sprinkles` below `ultra`, from either surface, removes exactly
those two lines again and leaves everything else in the config alone. A
`custom-shader` line you added yourself, pointing at a shader of your own,
is never touched in either direction.

This only happens when a Ghostty config is found at all, in the same
locations `bios theme use` already looks in; with none found, nothing is
written and nothing complains. On the command line, `--no-shaders` (on
`bios sprinkles <level>`, any level) skips this step entirely, in either
direction, for anyone who wants the sound but not the shaders; there is no
equivalent switch in `bios setup`, which always does both.

### The two config keys

```toml
ultra_chance = 20   # how often a cd earns a folder remark, in percent
ultra_volume = 0.5  # 0 to 1; 0 keeps the visuals and mutes the sound
```

`SPARKLEBIOS_ULTRA_CHANCE` and `SPARKLEBIOS_ULTRA_VOLUME` override them for
one run, the same tolerant way every other override on this page works: a
value out of range, or one that will not parse, reads as the default rather
than failing. See [config.md](config.md).

### What keeps happening

- **A jingle**, once, right after a shell starts.
- **A long command** plays `post_ok` or `post_fail` once it has run long
  enough for the finish line itself to print (`presence_after` seconds, and
  only when `presence` is on). With `presence` off, or before that threshold,
  a long command is silent: there is no fallback sound for it any more. This
  runs the same in all three shells.
- **A folder remark**, at `ultra_chance` percent on any `cd`: one dim line,
  chosen at random from four, and always a real fact about the directory you
  landed in (an entry count, the age of its last write, its git branch and
  uncommitted count, or that it is empty). Text only, and skipped under
  `NO_COLOR` like every other visual effect below.
- **Two ceremonies**: `Saving to CMOS... done.`, in cyan, after a `git
  commit` that succeeds, and the screen filling with falling glyphs for 1.2
  seconds after a `git push` that succeeds, or for 1.6 seconds the first
  time a command that failed last time succeeds and took two seconds or
  more. The rain waits between frames without spawning anything in zsh, and
  in bash from version 4 on; bash 3.2 (what macOS ships) and fish have no
  fractional builtin wait, so both spawn a `sleep` per frame, with fish
  capped at twenty frames for that reason.
- **A status flag**, `SPARKLEBIOS_STATUS`: `ERR 0x<hex>` after a command
  fails (Ctrl-C excluded), plus ` 66MHz` while Turbo is on, empty otherwise.
  Nothing is added to your prompt for you; wire it in yourself, for example
  `RPROMPT="$SPARKLEBIOS_STATUS"` in zsh, `PS1="$PS1\$SPARKLEBIOS_STATUS"` in
  bash, or a `fish_right_prompt` function that prints `$SPARKLEBIOS_STATUS`
  in fish.
- **A power-off**: on exit, the shutdown sound plays, and, on a real
  terminal, a bright line at the middle row shrinks to nothing over about
  300ms.

### One command

`ultra` also ships a screen of its own, run by hand rather than triggered
automatically:

- `bios screensaver` bounces the SPARKLEBIOS wordmark around the screen, DVD
  style, until you press a key. Where the terminal speaks the Kitty graphics
  protocol (kitty, Ghostty) it moves a real wordmark image; on iTerm2 and
  WezTerm, whose own image protocol has no way to move an already sent image
  without resending the whole picture, it draws the wordmark as six colour
  text instead.

It needs a terminal at least 80 columns by 24 rows, the same floor `bios
setup` uses, and it hands the terminal back exactly as it found it, on Esc,
Ctrl-C, or even a panic.

### Muting it

`ultra_volume = 0` (or `SPARKLEBIOS_ULTRA_VOLUME=0`) keeps every visual
effect and both screens, and silences only the sound: the detached player is
simply never started. Dropping `sprinkles` back to `full` keeps the POST
beep and loses the rest of this page; `off` turns it all off, same as ever.

## The never-flash promise

No sprinkle ever changes more than a quarter of the screen's cells in one
frame, and no single cell is ever made to change state more than three times
in one second. This is checked directly: a test diffs every frame a sprinkled
show actually draws, cell by cell, against the timings the player itself
uses.

## Turning it off

`sprinkles = "off"` in `config.toml`, or `bios sprinkles off`, or
`SPARKLEBIOS_SPRINKLES=off` for one run. With it off, no sprinkle code runs
at all past reading the value: the byte stream a boot writes is identical to
one from before sprinkles existed.
