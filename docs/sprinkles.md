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
| `ultra` | Everything `full` does, plus a whole day of it: sound, reactions to everyday commands and a screensaver. See [ultra](#ultra) below. |

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

- `post_ok`, `post_fail`: the finish line's own beep, good and bad.
- `floppy_seek`: a short seek, for `cd` and for a look around.
- `hdd_chatter`: the drive thinking, for a command that is taking a while.
- `hdd_spindown`: the drive settling, for a command that just finished.
- `modem`: the network noise, for anything that reaches outside the machine.
- `power_off`: the shutdown click, played once the shell exits.
- `jingle_<flavour>`: one small tune per built-in flavour (`unicorn`, `sumo`,
  `ninja`, `viking`, `luchador`, `yeti`, `raccoon`, `wizard`), played once a
  shell starts. A flavour you write yourself gets no jingle.

Playing one costs nothing running per prompt beyond starting the system's
own audio player, detached: `afplay` on macOS, or the first of `pw-play`,
`paplay` or `aplay` found on Linux. If none of those exist, nothing plays
and nothing complains either. The shell never waits on it.

### The two config keys

```toml
ultra_chance = 20   # how often an everyday command earns a reaction, in percent
ultra_volume = 0.5  # 0 to 1; 0 keeps the visuals and mutes the sound
```

`SPARKLEBIOS_ULTRA_CHANCE` and `SPARKLEBIOS_ULTRA_VOLUME` override them for
one run, the same tolerant way every other override on this page works: a
value out of range, or one that will not parse, reads as the default rather
than failing. See [config.md](config.md).

### What keeps happening

- **A jingle**, once, right after a shell starts.
- **Everyday commands** earn a sound at `ultra_chance` percent, unless a
  different figure is named here: `cd` into a git repository plays
  `floppy_seek` at 60 percent, `cd` anywhere else at 25 percent; `ls`, `la`,
  `ll` and `tree` at 10 percent; `git status`, `git log` and `git diff` at 30
  percent; `git pull`, `git fetch`, `curl`, `wget`, `npm i`, `pnpm i`,
  `cargo build` and `make` play `modem` at 50 percent; `rm`, `mv`, `cp`,
  `mkdir` and `touch` play `hdd_spindown` at 15 percent; `git push`, `ssh`,
  `scp` and `rsync` play `modem` every time.
- **A long command** plays `hdd_spindown` once it finishes, if it ran two
  seconds or more, and `post_ok` or `post_fail` instead once it has run long
  enough for the finish line itself to print (`presence_after` seconds, and
  only when `presence` is on; with `presence` off a long command still gets
  `hdd_spindown`, never the beep). In zsh, which can wait on a timer without
  spawning a process for it, a command still running after two seconds also
  starts `hdd_chatter` in the background while it thinks. bash and fish have
  no such builtin wait, so that one sound, and only that one, is zsh only;
  everything else on this page runs the same in all three shells.
- **A folder remark**, at `ultra_chance` percent on any `cd`: one dim line,
  chosen at random from four, and always a real fact about the directory you
  landed in (an entry count, the age of its last write, its git branch and
  uncommitted count, or that it is empty). Skipped under `NO_COLOR`, along
  with every other visual effect below; the sound is not.
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
