# SparkleBIOS shell hook. Printed by `bios init fish`.
# Add to ~/.config/fish/config.fish:
#   command -v bios >/dev/null 2>&1; and bios init fish | source
if status is-interactive; and command -v bios >/dev/null 2>&1
    set -l _sparklebios_typed (bios boot --hook)
    set -gx SPARKLEBIOS_BOOTED 1
    if test -n "$_sparklebios_typed"
        commandline -r -- $_sparklebios_typed
    end

    # `bios resume` prints a path. A command cannot change its caller's
    # working directory, so the function does the cd.
    function bios
        if test "$argv[1]" = resume
            set -l _sparklebios_dir (command bios resume $argv[2..])
            set -l _sparklebios_status $status
            if test $_sparklebios_status -ne 0
                return $_sparklebios_status
            end
            if test -n "$_sparklebios_dir"
                builtin cd -- $_sparklebios_dir
            end
        else
            command bios $argv
        end
    end
    # True while `name` is still fish's own version of a function, false once the user has
    # written their own. fish ships defaults for both the title and the missing command
    # handler, so `functions -q` can never tell us apart from fish itself. Declared here,
    # outside either block below, since the title and the missing-command handler are on
    # independent config keys and each needs it.
    function _sparklebios_is_fishs_own --argument-names name
        set -l file (functions --details $name 2>/dev/null)
        if test -z "$file"; or test "$file" = n/a
            return 0
        end
        if string match -q 'embedded:*' -- $file
            return 0
        end
        string match -q '*/share/fish/functions/*' -- $file
    end
    eval {{ULTRA}}

    # Set on its own, independent of presence: the tab title is its own config key. fish
    # owns the title through fish_title. Printing the escape from a prompt hook instead
    # would race with fish doing the same thing, so replace the function.
    if test {{TITLE}} = 1; and _sparklebios_is_fishs_own fish_title
        function fish_title
            printf '%s %s' {{TITLE_NAME}} (string replace -r "^$HOME" '~' -- $PWD)
        end
    end

    if test {{PRESENCE}} = 1
        # The mascot, as a placement of an image already sent to the terminal. Empty where the
        # terminal cannot draw one. Put it at the front of your prompt function:
        #   function fish_prompt; printf '%s' $SPARKLEBIOS_MASCOT; ...; end
        # The old placement is kept only long enough to compare against the new one: a second
        # `bios init fish | source` in the same shell (same flavour, same terminal) would
        # otherwise resend an image the terminal already has, every time it is run.
        set -l _sparklebios_prev_mascot "$SPARKLEBIOS_MASCOT"
        set -gx SPARKLEBIOS_MASCOT {{MASCOT}}
        # The image itself is sent once, here. Every prompt after that costs only the placement.
        set -l _sparklebios_image {{MASCOT_IMAGE}}
        if test -n "$_sparklebios_image"; and test "$_sparklebios_prev_mascot" != "$SPARKLEBIOS_MASCOT"
            printf '%s' $_sparklebios_image
        end

        function _sparklebios_took
            set -l s $argv[1]
            if test $s -lt 60
                printf '%ss' $s
            else if test $s -lt 3600
                printf '%sm %ss' (math "floor($s / 60)") (math "$s % 60")
            else
                printf '%sh %sm' (math "floor($s / 3600)") (math "floor(($s % 3600) / 60)")
            end
        end

        function _sparklebios_preexec --on-event fish_preexec
            set -g _sparklebios_started (date +%s)
        end

        function _sparklebios_precmd --on-event fish_prompt
            set -l status_was $status
            if set -q _sparklebios_started
                set -l elapsed (math (date +%s) - $_sparklebios_started)
                set -e _sparklebios_started
                # 130 is Ctrl-C. Somebody who stopped a command does not need it described.
                if test $elapsed -ge {{AFTER}}; and test $status_was -ne 130
                    set -l took (_sparklebios_took $elapsed)
                    # The line is an argument, never part of the format, so nothing a
                    # flavour writes is read as a printf directive or as shell.
                    if test $status_was -eq 0
                        printf '\e[90m%s\e[0m\n' {{DONE}}
                    else
                        printf '\e[90m%s\e[0m\n' {{FAILED}}
                    end
                end
            end
        end

        function _sparklebios_exit --on-event fish_exit
            printf '%s\n' {{GOODBYE}}
        end

        # Only when nothing else already answers for a missing command. fish always has an
        # answer of its own, so this asks whether the answer is still fish's rather than whether
        # one exists at all.
        if _sparklebios_is_fishs_own fish_command_not_found
            function fish_command_not_found
                printf '%s: %s\n' {{NOT_FOUND}} $argv[1] >&2
                return 127
            end
        end

    end

    functions -e _sparklebios_is_fishs_own
end
