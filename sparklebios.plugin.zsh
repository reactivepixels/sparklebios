# SparkleBIOS as a zsh plugin, for oh-my-zsh, antidote, zinit, zplug, sheldon and the like.
# It does what the one line at the end of ~/.zshrc does, and nothing else:
#   command -v bios >/dev/null 2>&1 && eval "$(bios init zsh)"
#
# This installs the shell hook, not the binary. Install `bios` first (README, Install). Without
# it this file does nothing at all, not even a message, so a plugin list shared between
# machines stays quiet on the ones that lack it.
#
# Load order matters a little, which is why the README says the END of your rc file. The hook
# draws the boot screen as it loads, defines `command_not_found_handler` only if nothing else
# has, and sets $SPARKLEBIOS_MASCOT for a `PROMPT="$SPARKLEBIOS_MASCOT$PROMPT"` line that has to
# come after it. So list this plugin last: after your prompt theme, and after any plugin that
# prints at startup or sets PROMPT. It is not deferred to the first prompt on purpose, because
# that would leave $SPARKLEBIOS_MASCOT empty when your rc file reads it.
#
# `emulate -L zsh` keeps a manager's option choices (KSH_ARRAYS, SH_WORD_SPLIT and so on) out
# of the hook. The hook itself returns quietly in a shell that is not interactive.
emulate -L zsh
if command -v bios >/dev/null 2>&1; then
  eval "$(bios init zsh)"
fi
