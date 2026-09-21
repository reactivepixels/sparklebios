# Presence

The boot is the introduction. Away from it, the chosen flavour speaks in six
small moments through the day, so choosing a flavour is rewarded more than
once a morning.

## Nothing spawns `bios` per prompt

`bios init <shell>` writes the flavour's finished strings into the hook text
it prints. The shell reads that text once, at startup, so it already holds
every word it will ever say: a tab title, a finish line, a goodbye, a typo
answer. None of it costs a process launch, because none of it launches a
process. The one exception is the mascot in a starship prompt, covered below,
and even that only runs if you choose to wire it in.

## ultra shares this hook

`sprinkles = "ultra"` (see [sprinkles.md](sprinkles.md#ultra)) writes its own
block into the same hook `bios init` prints, and the two features lean on
each other in a couple of places worth knowing about. Ultra's own beep for a
long command, `post_ok` or `post_fail`, only plays alongside moment 3's
finish line below: it needs `presence_after` seconds to have passed and
`presence` to be turned on, the same as the finish line itself. With
`presence` off, a long command under `ultra` gets no sound at all: there is
no fallback for it. Turn `presence` off and `ultra`'s everyday reactions,
its jingle, its ceremonies and its power-off still run; only the piece that
rides on the finish line changes.

## The six moments

1. **Tab title.** The window title becomes `{title} {cwd}`, for example
   `SHINOBI-0 ~/Code/sparklebios`, with `$HOME` shown as `~`. Set on every
   prompt, skipped on `TERM=dumb` and `TERM=linux`. Config key: `title`.
2. **The mascot in the prompt.** `SPARKLEBIOS_MASCOT` holds what goes in
   front of your prompt. Put it there and it costs only a `printf` of a
   string the shell already holds.

   What is in it depends on the terminal. Under the kitty protocol, which
   Ghostty and kitty speak, the hook sends the picture once at startup and
   the variable holds the short escape that places it, about 40 bytes a
   prompt. iTerm2 and WezTerm have no stored image and no placement, so the
   variable holds the picture itself, about 3KB reprinted each prompt: more
   bytes, still no process. Anywhere else there is no image at all.

   A boot streak of two days or more is added after the mascot, as `12d`. In
   a terminal that cannot draw images that is all the variable holds, so a
   prompt in Terminal.app gets something out of this too. With no image and
   no streak the variable is empty and an unmodified prompt is unaffected.

   The streak is read once, when the hook is generated, so it is fixed for
   that shell's lifetime. A tab left open across midnight shows yesterday's
   number until you open a new one. That is the price of never spawning
   anything per prompt.
3. **The finish line.** Once a command has run for at least `presence_after`
   seconds, the next prompt prints a dim line saying so, one of two ways
   depending on whether it succeeded. A command stopped with Ctrl-C is left
   alone: somebody who stopped it does not need it described.
4. **Goodbye.** The flavour says one line when an interactive shell exits.
5. **A typo answered.** Type a command that does not exist and the flavour
   answers instead of the shell's own "command not found". If you already
   define your own handler, `bios init` leaves it alone and says nothing
   here. The typo answer needs bash 4 or newer; macOS ships 3.2, so on a Mac
   use zsh or install a newer bash.
6. **In character.** `bios refresh`, `bios resume` and `bios use <flavour>`
   each print their own in-character line alongside their normal output.

## What fish does differently

fish ships its own `fish_title` and `fish_command_not_found`, so unlike zsh and
bash there is always something there before the hook runs. The hook replaces
each of them only while it is still fish's own version, and leaves one you
wrote yourself alone. The title goes through `fish_title` rather than a prompt
hook, because fish sets the title itself and printing the escape from a prompt
would race with it.

## A flavour's words are text, never code

A flavour is a TOML file on disk, including any you write. `bios init` writes
each line into the hook as quoted text and passes it to `printf` as an
argument, so a line holding a `$`, a backtick or a quote prints exactly as
written instead of running. The only thing spliced in unquoted is the
duration, which the shell works out itself.

## Config keys

```toml
# Presence: the flavour speaks after long commands, on typos and at exit. false is silence.
presence = true
# Seconds a command must run before the flavour comments on it.
presence_after = 10
# Tab title: the flavour's board name and the current folder. false leaves the title alone.
title = true
```

`presence = false` silences moments 2 through 6 in one line: no finish line,
no goodbye, no typo answer, and no in-character line from `refresh`,
`resume` or `use`. It leaves the tab title alone, since `title` is its own
key. `SPARKLEBIOS_BOOT=0` silences presence for that shell too, the same way
it silences the boot.

## Wiring it into your prompt

zsh, at the end of `~/.zshrc`, after the line that evals `bios init zsh`:

```sh
PROMPT="$SPARKLEBIOS_MASCOT$PROMPT"
```

Double quotes on purpose. The placement never changes during a session, so it
is expanded once, here. Single quotes would need `setopt PROMPT_SUBST`, and
without that option the prompt shows nothing at all.

bash, at the end of `~/.bashrc`, after the line that evals `bios init bash`:

```sh
PS1="$SPARKLEBIOS_MASCOT$PS1"
```

fish, in `~/.config/fish/config.fish`, after the line that sources
`bios init fish`, inside your own prompt function:

```fish
function fish_prompt
    printf '%s' $SPARKLEBIOS_MASCOT
    # the rest of your prompt
end
```

## starship

starship spawns every custom command it runs, so there is no hook to bake
the mascot into the way the other shells do. `bios prompt` exists for this
case: it prints the same placement `bios init` would have baked in, and, on
its first call in a shell, the image transmission that placement needs.

```toml
[custom.sparklebios_mascot]
command = "bios prompt"
when = true
shell = ["bash", "--noprofile", "--norc"]
format = "$output"
```

`bios prompt` costs a few milliseconds per prompt, since starship has to
spawn it, unlike the other shells where the cost is a `printf` of a string
already in memory. It always exits 0, and prints nothing at all on any
error, or wherever the terminal cannot draw an image: a prompt command must
never be able to break a prompt.

Sending the same image on every prompt is wasted work once the terminal
already has it. Once your first prompt has drawn, set
`SPARKLEBIOS_PROMPT_IMG=1` in your shell's rc file and `bios prompt` sends
only the placement from then on:

```sh
export SPARKLEBIOS_PROMPT_IMG=1
```

## Silencing each moment on its own

| Moment | How to silence it |
|---|---|
| Tab title | `title = false` |
| The mascot | Don't add `$SPARKLEBIOS_MASCOT` to your prompt, or remove the `bios prompt` module from starship |
| Finish line | `presence = false`, or raise `presence_after` past any command you run |
| Goodbye | `presence = false` |
| Typo answered | Define your own `command_not_found_handler` (zsh), `command_not_found_handle` (bash) or `fish_command_not_found` (fish) before `bios init` runs, or `presence = false` |
| `refresh` / `resume` / `use` lines | `presence = false` |

## A restored tab

A tab restored from hibernation by your terminal or your OS may come back
without the mascot image, since the image lived in that terminal session's
memory and hibernation does not replay it. Open a new tab and it is there
again.
