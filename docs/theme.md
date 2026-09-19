# The theme: Rainbows and Unicorns

A serious 1985 colour terminal theme for Ghostty. The name is the only joke.
Four variants share one idea: the rainbow is simply the ANSI colours your tools
were already going to use.

Contrast figures are WCAG ratios against that variant's background.

## Decisions

- **Glass, not black.** The lead background is the colour of a switched-off tube with a breath of violet, so warm white text sits in front of it.
- **All six stripes are addressable.** Orange rides in bright red (ANSI 9), so the classic six-stripe order is ANSI 2, 3, 9, 1, 5, 4. Any tool that prints those in sequence draws the stripes without knowing it. `neigh`'s default palette relies on this.
- **Nothing was thrown away.** The original stripe purple (`#963D97`) is too dark to read on glass, so it became the reverse-video selection colour.
- **The cursor is the horn.** A gold block, blinking. The one place the theme winks.
- **Bold is not bright.** Set `bold-is-bright = false`. Modern tools assume bold means weight.
- **A pair.** Paper White is the light half so Ghostty can follow the system appearance.

## Variants

### `rainbows-and-unicorns` (Six Stripes, the lead)

The six-colour stripe set of 1977 mapped onto ANSI and tuned as light coming
through glass: slightly softened, never neon. Every chromatic colour clears 4.5:1.

| Role | Value | | Role | Value |
|---|---|---|---|---|
| background | `#0E0E13` | | foreground | `#E6E3D8` (15.0) |
| cursor-color | `#FCB827` | | cursor-text | `#0E0E13` |
| selection-background | `#963D97` | | selection-foreground | `#FFFDF5` |

| # | Normal | Value | Contrast | # | Bright | Value | Contrast |
|---|---|---|---|---|---|---|---|
| 0 | black | `#23232D` | 1.2 | 8 | black | `#6F6E7D` | 3.9 |
| 1 | red | `#E0453F` | 4.7 | 9 | red (orange) | `#F5821F` | 7.4 |
| 2 | green | `#62BB47` | 8.0 | 10 | green | `#8AD46B` | 10.7 |
| 3 | yellow | `#FCB827` | 11.0 | 11 | yellow | `#FFD35C` | 13.5 |
| 4 | blue | `#1D9BE3` | 6.3 | 12 | blue | `#5DB9F0` | 8.9 |
| 5 | magenta | `#BB62C0` | 5.2 | 13 | magenta | `#D68ADB` | 7.8 |
| 6 | cyan | `#35C9BE` | 9.4 | 14 | cyan | `#6FDDD3` | 11.9 |
| 7 | white | `#C9C6BC` | 11.3 | 15 | white | `#FFFDF5` | 18.9 |

### `rainbows-and-unicorns-paper` (Paper White, the light half)

Page-white in the manner of a 1984 compact computer, stripes darkened to ink,
selection as true reverse video.

| Role | Value | | Role | Value |
|---|---|---|---|---|
| background | `#EFEDE6` | | foreground | `#1B1B1F` (14.7) |
| cursor-color | `#1B1B1F` | | cursor-text | `#EFEDE6` |
| selection-background | `#1B1B1F` | | selection-foreground | `#EFEDE6` |

| # | Normal | Value | # | Bright | Value |
|---|---|---|---|---|---|
| 0 | black | `#1B1B1F` | 8 | black | `#8B8A84` |
| 1 | red | `#C2272D` | 9 | red (orange) | `#B04E00` |
| 2 | green | `#2F7D1F` | 10 | green | `#33851F` |
| 3 | yellow | `#8A6100` | 11 | yellow | `#946A00` |
| 4 | blue | `#0B6FA6` | 12 | blue | `#1279B5` |
| 5 | magenta | `#8A2F8C` | 13 | magenta | `#A8429F` |
| 6 | cyan | `#0B7F78` | 14 | cyan | `#0B807A` |
| 7 | white | `#5E5D58` | 15 | white | `#FFFFFF` |

### `rainbows-and-unicorns-ega` (EGA '85)

The 1984 sixteen-colour PC palette, brown included. Hues kept, luminance of the
normal row corrected so blue on black is readable. The bright row is left raw.
Selection is reverse video, the period-correct way.

| Role | Value | | Role | Value |
|---|---|---|---|---|
| background | `#050507` | | foreground | `#AAAAAA` (8.8) |
| cursor-color | `#AAAAAA` | | cursor-text | `#050507` |
| selection-background | `#AAAAAA` | | selection-foreground | `#050507` |

| # | Normal | Value | # | Bright | Value |
|---|---|---|---|---|---|
| 0 | black | `#1A1A1F` | 8 | black | `#6A6A72` |
| 1 | red | `#CC3B3B` | 9 | red | `#FF5555` |
| 2 | green | `#2FB84A` | 10 | green | `#55FF55` |
| 3 | yellow (brown) | `#C2782E` | 11 | yellow | `#FFFF55` |
| 4 | blue | `#4A6CF0` | 12 | blue | `#7B7BFF` |
| 5 | magenta | `#C24BC2` | 13 | magenta | `#FF55FF` |
| 6 | cyan | `#2BB8B8` | 14 | cyan | `#55FFFF` |
| 7 | white | `#AAAAAA` | 15 | white | `#FFFFFF` |

### `rainbows-and-unicorns-workbench` (Workbench 1.0)

A 1985 home computer desktop: blue ground, white text, orange cursor. The blue is
a shade deeper than the historical `#0055AA` so text holds up all day, and the
rainbow goes pastel because on a blue ground it has to.

| Role | Value | | Role | Value |
|---|---|---|---|---|
| background | `#00408F` | | foreground | `#FFFFFF` (9.9) |
| cursor-color | `#FF8800` | | cursor-text | `#00204A` |
| selection-background | `#FFFFFF` | | selection-foreground | `#00408F` |

| # | Normal | Value | # | Bright | Value |
|---|---|---|---|---|---|
| 0 | black | `#000022` | 8 | black | `#7FA3CF` |
| 1 | red | `#FFA79B` | 9 | red (orange) | `#FFB066` |
| 2 | green | `#A6EE8E` | 10 | green | `#C8F7B6` |
| 3 | yellow | `#FFD978` | 11 | yellow | `#FFE9A6` |
| 4 | blue | `#9AD2FF` | 12 | blue | `#C2E4FF` |
| 5 | magenta | `#F0B0F2` | 13 | magenta | `#F7CFF8` |
| 6 | cyan | `#8CECE2` | 14 | cyan | `#BCF5EE` |
| 7 | white | `#DCE6F2` | 15 | white | `#FFFFFF` |

## Using it

Copy the files from `themes/` into your Ghostty themes directory
(`~/.config/ghostty/themes/`), then in your Ghostty config:

```
theme = dark:rainbows-and-unicorns,light:rainbows-and-unicorns-paper
cursor-style = block
cursor-style-blink = true
bold-is-bright = false
```

Suggested type: IBM Plex Mono for the straight face. For the full 1985, a bitmap
face such as Departure Mono.
