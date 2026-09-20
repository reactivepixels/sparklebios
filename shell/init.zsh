# SparkleBIOS shell hook. Printed by `bios init zsh`.
# Add to the END of ~/.zshrc:
#   command -v bios >/dev/null 2>&1 && eval "$(bios init zsh)"
if [[ -o interactive ]] && command -v bios >/dev/null 2>&1; then
  _sparklebios_typed="$(bios boot --hook)"
  export SPARKLEBIOS_BOOTED=1
  [[ -n "$_sparklebios_typed" ]] && print -z -- "$_sparklebios_typed"
  unset _sparklebios_typed

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
