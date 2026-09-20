# The theme: Rainbows and Unicorns

A serious 1985 colour terminal theme for Ghostty. The name is the only joke.
Ten variants share one idea: the rainbow is simply the ANSI colours your tools
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

### `rainbows-and-unicorns-mane` (Mane)

The palette of the unicorn itself. Every colour is lifted from the sprite: the six
bands of the mane, the pink of its ear in bright magenta, the lavender of its
shading as plain white, the gold of the horn as the cursor. Made to sit under the
`pc95` boot screen, which paints no background of its own. Opt in; it is not the default.

| Role | Value | | Role | Value |
|---|---|---|---|---|
| background | `#14131A` | | foreground | `#E9E7F2` (15.1) |
| cursor-color | `#FFD65A` | | cursor-text | `#14131A` |
| selection-background | `#9230AA` | | selection-foreground | `#FFFFFF` |

| # | Normal | Value | # | Bright | Value |
|---|---|---|---|---|---|
| 0 | black | `#24222E` | 8 | black | `#6E6A82` |
| 1 | red | `#EB413A` | 9 | red (orange) | `#FA821E` |
| 2 | green | `#68C44A` | 10 | green | `#8FDB74` |
| 3 | yellow | `#FEDE3C` | 11 | yellow | `#FFE978` |
| 4 | blue | `#3092E2` | 12 | blue | `#62B0F0` |
| 5 | magenta | `#B866D6` | 13 | magenta (pink) | `#F0A0BE` |
| 6 | cyan | `#45C8C0` | 14 | cyan | `#7FE0D8` |
| 7 | white (lavender) | `#C8CAE4` | 15 | white | `#FFFFFF` |

### `rainbows-and-unicorns-miami` (Miami)

Pastel neon on deep navy: flamingo pink, pool teal, a sunset that refuses to end. The loud one,
and the one to pair with anything that has a mane.

| Role | Value | | Role | Value |
|---|---|---|---|---|
| background | `#1B1433` | | foreground | `#F9ECF5` (15.4) |
| cursor-color | `#FF6FD8` | | cursor-text | `#1B1433` |
| selection-background | `#FF6FD8` | | selection-foreground | `#1B1433` |

| # | Normal | Value | # | Bright | Value |
|---|---|---|---|---|---|
| 0 | black | `#2A2148` | 8 | black | `#7A6C9C` |
| 1 | red | `#FF5C8A` | 9 | red (orange) | `#FF9E64` |
| 2 | green | `#7CF0A8` | 10 | green | `#A8F7C6` |
| 3 | yellow | `#FFE27A` | 11 | yellow | `#FFEFAE` |
| 4 | blue | `#6FB7FF` | 12 | blue | `#A3D2FF` |
| 5 | magenta | `#FF6FD8` | 13 | magenta | `#FFA3E8` |
| 6 | cyan | `#3FE0D0` | 14 | cyan | `#8AF0E4` |
| 7 | white | `#D9CBE8` | 15 | white | `#FFFFFF` |

### `rainbows-and-unicorns-arcade` (Arcade)

Cabinet neon on true black. Every colour is turned up as far as it goes, the way a marquee is.
The cursor is the green of a vector monitor.

| Role | Value | | Role | Value |
|---|---|---|---|---|
| background | `#000000` | | foreground | `#EDEDED` (17.9) |
| cursor-color | `#39FF14` | | cursor-text | `#000000` |
| selection-background | `#39FF14` | | selection-foreground | `#000000` |

| # | Normal | Value | # | Bright | Value |
|---|---|---|---|---|---|
| 0 | black | `#1A1A1A` | 8 | black | `#6E6E6E` |
| 1 | red | `#FF2A4D` | 9 | red (orange) | `#FF8A1F` |
| 2 | green | `#39FF14` | 10 | green | `#8CFF75` |
| 3 | yellow | `#FFF01F` | 11 | yellow | `#FFF777` |
| 4 | blue | `#1F8BFF` | 12 | blue | `#6FB4FF` |
| 5 | magenta | `#FF2CF0` | 13 | magenta | `#FF7DF5` |
| 6 | cyan | `#00F0FF` | 14 | cyan | `#7AF7FF` |
| 7 | white | `#C8C8C8` | 15 | white | `#FFFFFF` |

### `rainbows-and-unicorns-vhs` (VHS)

The blue of a video recorder with nothing to play, warm white text, and colours a little washed
out, as if the tape has been rented a few times.

| Role | Value | | Role | Value |
|---|---|---|---|---|
| background | `#101B45` | | foreground | `#F2EBDD` (14.0) |
| cursor-color | `#F2EBDD` | | cursor-text | `#101B45` |
| selection-background | `#F2EBDD` | | selection-foreground | `#101B45` |

| # | Normal | Value | # | Bright | Value |
|---|---|---|---|---|---|
| 0 | black | `#0A1230` | 8 | black | `#6F7BA8` |
| 1 | red | `#FF6B6B` | 9 | red (orange) | `#FFA862` |
| 2 | green | `#8FE3A0` | 10 | green | `#B5F0C1` |
| 3 | yellow | `#F7D774` | 11 | yellow | `#FBE6A3` |
| 4 | blue | `#7FB2FF` | 12 | blue | `#ABCDFF` |
| 5 | magenta | `#D59CFF` | 13 | magenta | `#E5C2FF` |
| 6 | cyan | `#7FE3E0` | 14 | cyan | `#AEF0EE` |
| 7 | white | `#D6D0C2` | 15 | white | `#FFFFFF` |

### `rainbows-and-unicorns-den` (Den)

Wood panelling, amber lamps and a carpet nobody chose. Warm browns and one good amber, for
long evenings in front of the family computer.

| Role | Value | | Role | Value |
|---|---|---|---|---|
| background | `#1E150D` | | foreground | `#EBD9B8` (13.0) |
| cursor-color | `#E0A030` | | cursor-text | `#1E150D` |
| selection-background | `#E0A030` | | selection-foreground | `#1E150D` |

| # | Normal | Value | # | Bright | Value |
|---|---|---|---|---|---|
| 0 | black | `#2E2115` | 8 | black | `#7D6A50` |
| 1 | red | `#D9583B` | 9 | red (orange) | `#D98036` |
| 2 | green | `#9DB85C` | 10 | green | `#BBD47E` |
| 3 | yellow | `#E0A030` | 11 | yellow | `#F0BE5C` |
| 4 | blue | `#6F9FB8` | 12 | blue | `#95BDD2` |
| 5 | magenta | `#B87A9A` | 13 | magenta | `#D29CB8` |
| 6 | cyan | `#7FB0A0` | 14 | cyan | `#A2CDBF` |
| 7 | white | `#CDBB98` | 15 | white | `#FFF3DA` |

### `rainbows-and-unicorns-sorbet` (Sorbet)

The soft one. Six pastels on deep plum: pink, mint, butter, periwinkle, lilac and aqua, with peach
riding in bright red where the other variants keep their orange. Every colour clears 9:1 on the
plum, so it is gentle to look at and still easy to read all day.

| Role | Value | | Role | Value |
|---|---|---|---|---|
| background | `#241B2F` | | foreground | `#F3E8F5` (13.9) |
| cursor-color | `#FF9EB5` | | cursor-text | `#241B2F` |
| selection-background | `#D7B3FF` | | selection-foreground | `#241B2F` |

| # | Normal | Value | # | Bright | Value |
|---|---|---|---|---|---|
| 0 | black | `#3A2D48` | 8 | black | `#8A789E` |
| 1 | red (pink) | `#FF9EB5` | 9 | red (peach) | `#FFBE98` |
| 2 | green (mint) | `#A8E6C3` | 10 | green | `#C6F3D9` |
| 3 | yellow (butter) | `#FFE8A3` | 11 | yellow | `#FFF2C7` |
| 4 | blue (periwinkle) | `#A7C7FF` | 12 | blue | `#C7DBFF` |
| 5 | magenta (lilac) | `#D7B3FF` | 13 | magenta | `#E8D1FF` |
| 6 | cyan (aqua) | `#9FE8E6` | 14 | cyan | `#C5F4F2` |
| 7 | white | `#E4D6EC` | 15 | white | `#FFFFFF` |

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

## Beyond the sixteen colours

The cursor trail shader, the tool colours and the starship palettes live in
[`extras/`](../extras/README.md). All of them are optional, and the tool colours
work by index, so they follow every variant above.

`bios theme use <name>` also points an existing starship prompt at the matching
palette, `rainbows_and_unicorns_auto` for every variant except Paper White, which
gets `rainbows_and_unicorns_paper`. This only happens when `~/.config/starship.toml`
(or `$STARSHIP_CONFIG`) already has a top level `palette =` line: SparkleBIOS never
creates a starship config or touches one without that line. Pass `--no-prompt` to
skip it.
