# Sprinkles

An optional delight layer on the once-a-day animated boot. Off by default,
because not everyone wants sprinkles.

## The dial

One key, `sprinkles`, three values:

| Value | What it does |
|---|---|
| `off` (default) | No sprinkle code runs. |
| `light` | Text effects: a shimmer, a twinkle, and (only on a streak milestone) a stripe sweep. |
| `full` | Everything `light` does, plus sound: the POST beep and the beep codes. |

Set it with `bios sprinkles <off\|light\|full>`, or run `bios sprinkles` with
no argument to print the level currently in effect. `SPARKLEBIOS_SPRINKLES`
overrides the config file for one run, the same way `SPARKLEBIOS_GRAPHICS`
overrides `graphics`.

Sprinkles only ever run in the once-a-day animated show. A fast, quiet, or
off boot is untouched, byte for byte, whatever the dial is set to.

## What `light` draws

- **Shimmer.** Once the firmware line finishes, a 3 cell highlight sweeps
  across it left to right, once, then the line returns to its normal style.
- **Twinkle.** While the memory count runs, up to three cells in the blank
  margin beside the mascot show a star, then a plus, then a full stop, then
  nothing, and nothing again after that.
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
