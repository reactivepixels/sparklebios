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
end
