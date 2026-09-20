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
  # The mascot, as a placement of an image already sent to the terminal. Empty where the
  # terminal cannot draw one. Put it at the front of your prompt, in double quotes, AFTER this
  # line and after anything else that sets PS1:
  #   PS1="$SPARKLEBIOS_MASCOT$PS1"
  # Double quotes on purpose: the placement never changes, so it is expanded once, here.
  export SPARKLEBIOS_MASCOT={{MASCOT}}
  # The image itself is sent once, here. Every prompt after that costs only the placement.
  _sparklebios_image={{MASCOT_IMAGE}}
  [ -n "$_sparklebios_image" ] && printf '%s' "$_sparklebios_image"
  unset _sparklebios_image

  if [[ {{PRESENCE}} == 1 ]]; then
    _sparklebios_took() {
      local s=$1
      if (( s < 60 )); then
        printf '%ss' "$s"
      elif (( s < 3600 )); then
        printf '%sm %ss' "$(( s / 60 ))" "$(( s % 60 ))"
      else
        printf '%sh %sm' "$(( s / 3600 ))" "$(( (s % 3600) / 60 ))"
      fi
    }

    # bash has no preexec, so the timer starts from the DEBUG trap and is armed once per
    # command rather than once per line of a compound one.
    _sparklebios_preexec() {
      [[ -n "$COMP_LINE" ]] && return
      [[ "$BASH_COMMAND" == _sparklebios_precmd ]] && return
      [[ -z "$_sparklebios_started" ]] && _sparklebios_started=$SECONDS
    }

    _sparklebios_precmd() {
      local status_was=$?
      if [[ {{TITLE}} == 1 && "$TERM" != dumb && "$TERM" != linux ]]; then
        printf '\e]0;%s %s\a' {{TITLE_NAME}} "${PWD/#$HOME/~}"
      fi
      if [[ -n "$_sparklebios_started" ]]; then
        local elapsed=$(( SECONDS - _sparklebios_started ))
        unset _sparklebios_started
        # 130 is Ctrl-C. Somebody who stopped a command does not need it described.
        if (( elapsed >= {{AFTER}} )) && (( status_was != 130 )); then
          local took
          took="$(_sparklebios_took $elapsed)"
          # The line is an argument, never part of the format, so nothing a flavour writes
          # is read as a printf directive or as shell.
          if (( status_was == 0 )); then
            printf '\e[90m%s\e[0m\n' {{DONE}}
          else
            printf '\e[90m%s\e[0m\n' {{FAILED}}
          fi
        fi
      fi
    }

    trap '_sparklebios_preexec' DEBUG
    PROMPT_COMMAND="_sparklebios_precmd${PROMPT_COMMAND:+; $PROMPT_COMMAND}"
    trap {{GOODBYE_TRAP}} EXIT

    # Only when nothing else already answers for a missing command.
    if ! declare -F command_not_found_handle >/dev/null 2>&1; then
      command_not_found_handle() {
        printf '%s: %s\n' {{NOT_FOUND}} "$1" >&2
        return 127
      }
    fi
  fi
fi
