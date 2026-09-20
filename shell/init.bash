# SparkleBIOS shell hook. Printed by `bios init bash`.
# Add to the END of ~/.bashrc:
#   command -v bios >/dev/null 2>&1 && eval "$(bios init bash)"
#
# Bash has no equivalent of zsh's `print -z`. READLINE_LINE only does
# anything from inside a `bind -x` key binding while readline is already
# reading a line, and this hook runs at shell startup, before readline's
# prompt loop has begun, so there is no way here to push keys typed during
# the boot show back onto the command line. They are read and discarded
# rather than faked back.
if [[ $- == *i* ]] && command -v bios >/dev/null 2>&1; then
  bios boot --hook >/dev/null
  export SPARKLEBIOS_BOOTED=1

  # `bios resume` prints a path. A command cannot change its caller's working
  # directory, so the function does the cd.
  bios() {
    if [[ "$1" == resume ]]; then
      local _sparklebios_dir
      _sparklebios_dir="$(command bios resume "${@:2}")" || return $?
      if [[ -n "$_sparklebios_dir" ]]; then
        builtin cd -- "$_sparklebios_dir"
      fi
    else
      command bios "$@"
    fi
  }
fi
