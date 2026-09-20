# Extras

Optional pieces that sit beside the theme. None of them need the `bios` binary,
and none of them are on by default.

| File | What it does |
|---|---|
| [shaders/cursor-trail.glsl](shaders/cursor-trail.glsl) | A Ghostty shader. When the cursor jumps it leaves a short six-stripe trail that retracts into it |
| [shaders/crt.glsl](shaders/crt.glsl) | A Ghostty shader. Scanlines, a slight curve, a soft glow on bright text and a vignette. Nothing animates, so nothing flickers |
| [tools/colours.zsh](tools/colours.zsh) | Makes `ls`, `eza`, `bat`, `fzf`, `man`, `less`, `grep` and zsh completion follow the terminal's sixteen colours |
| [tools/delta.gitconfig](tools/delta.gitconfig) | Git diffs through `delta`, drawn from the same sixteen colours |
| [starship-palette.toml](starship-palette.toml) | Four palettes for starship's Gruvbox Rainbow preset: `rainbows_and_unicorns`, `rainbows_and_unicorns_mane`, `rainbows_and_unicorns_auto` and `rainbows_and_unicorns_paper`. `bios theme use` switches an opted-in prompt between `_auto`, which uses the terminal's own colours by number so it follows whichever theme variant is active, and `_paper`, for Paper White alone |

## The idea

A terminal theme only sets sixteen colours. What you actually look at all day is
tool output, and most tools ship with their own hard coded colours. Everything in
`tools/` does the opposite: it tells each tool to use the terminal's own palette
by index. Nothing here names a hex value, so it matches whichever variant of the
theme you run, and it changes when you change variant.

## Turning things on

The cursor trail, in your Ghostty config:

```
custom-shader = /path/to/extras/shaders/cursor-trail.glsl
```

The tool colours, at the end of `~/.zshrc`:

```
source /path/to/extras/tools/colours.zsh
```

The diffs, in `~/.gitconfig` (needs `delta` installed):

```
[include]
    path = /path/to/extras/tools/delta.gitconfig
```

For editors, choose a theme that uses terminal colours: `colorscheme default`
with `set notermguicolors` in vim, or an ANSI based theme in helix.
