# SparkleBIOS shell hook. Printed by `bios init zsh`.
# Add to the END of ~/.zshrc:
#   command -v bios >/dev/null 2>&1 && eval "$(bios init zsh)"
#
# Every line the flavour says is written into this text when it is printed, so nothing here
# spawns `bios` to speak. The only cost per prompt is a printf of a string the shell already
# holds.
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
  eval {{ULTRA}}

  # Set on its own, independent of presence: the tab title is its own config key.
  if [[ {{TITLE}} == 1 ]]; then
    _sparklebios_title() {
      if [[ "$TERM" != dumb && "$TERM" != linux ]]; then
        printf '\e]0;%s %s\a' {{TITLE_NAME}} "${PWD/#$HOME/~}"
      fi
    }
    autoload -Uz add-zsh-hook 2>/dev/null
    if (( $+functions[add-zsh-hook] )); then
      add-zsh-hook precmd _sparklebios_title
    fi
  fi

  if [[ {{PRESENCE}} == 1 ]]; then
    # The mascot, as a placement of an image already sent to the terminal. Empty where the
    # terminal cannot draw one. Put it at the front of your prompt, in double quotes, AFTER this
    # line and after anything else that sets PROMPT:
    #   PROMPT="$SPARKLEBIOS_MASCOT$PROMPT"
    # Double quotes on purpose. The placement never changes, so it is expanded once here rather
    # than on every prompt, and that way it needs no PROMPT_SUBST.
    #
    # The old placement is kept only long enough to compare against the new one: a second
    # `eval "$(bios init zsh)"` in the same shell (same flavour, same terminal) would
    # otherwise resend an image the terminal already has, every time it is run.
    _sparklebios_prev_mascot="${SPARKLEBIOS_MASCOT-}"
    export SPARKLEBIOS_MASCOT={{MASCOT}}
    # The image itself is sent once, here. Every prompt after that costs only the placement.
    _sparklebios_image={{MASCOT_IMAGE}}
    if [[ -n "$_sparklebios_image" && "$_sparklebios_prev_mascot" != "$SPARKLEBIOS_MASCOT" ]]; then
      printf '%s' "$_sparklebios_image"
    fi
    unset _sparklebios_image _sparklebios_prev_mascot

    _sparklebios_took() {
      local s=$1
      if (( s < 60 )); then
        print -r -- "${s}s"
      elif (( s < 3600 )); then
        print -r -- "$((s / 60))m $((s % 60))s"
      else
        print -r -- "$((s / 3600))h $(((s % 3600) / 60))m"
      fi
    }

    _sparklebios_preexec() {
      _sparklebios_started=$EPOCHSECONDS
    }

    _sparklebios_precmd() {
      local status_was=$?
      if [[ -n "${_sparklebios_started:-}" ]]; then
        local elapsed=$(( EPOCHSECONDS - _sparklebios_started ))
        unset _sparklebios_started
        # Out of the arithmetic so this file is still a parseable shell script before
        # the slots are filled in.
        local after={{AFTER}}
        # 130 is Ctrl-C. Somebody who stopped a command does not need it described.
        if (( elapsed >= after )) && (( status_was != 130 )); then
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

    _sparklebios_exit() {
      print -r -- {{GOODBYE}}
    }

    autoload -Uz add-zsh-hook 2>/dev/null
    zmodload zsh/datetime 2>/dev/null
    if (( $+functions[add-zsh-hook] )); then
      add-zsh-hook preexec _sparklebios_preexec
      add-zsh-hook precmd _sparklebios_precmd
      add-zsh-hook zshexit _sparklebios_exit
    fi

    # Only when nothing else already answers for a missing command.
    if (( ! $+functions[command_not_found_handler] )); then
      command_not_found_handler() {
        printf '%s: %s\n' {{NOT_FOUND}} "$1" >&2
        return 127
      }
    fi
  fi
fi
