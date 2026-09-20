# Checks

A check runs a probe and yields a finding. A finding is `{id, severity, ttl}`:
an `id` such as `boot_order`, a `severity` of `info`, `warn` or `fail`, and a
`ttl` in seconds, the length of time the finding may still be shown once the
cache that holds it was generated.

## No probe ever runs while a shell is starting

The boot path reads one JSON file and nothing else. All probing happens in
`bios refresh`, a separate command that `bios boot` spawns detached and never
waits on. A slow or hanging probe can never slow down a shell starting up.

## The eight checks

| Check | Probes | Commands | Finding ids | Slots set | Severity | ttl |
|---|---|---|---|---|---|---|
| Boot device order | The git repositories you touched most recently, and whether the most recently touched one has uncommitted changes | `git --no-optional-locks status --porcelain`, run only in the single most recent repository | `boot_order`, `boot_dirty` | `boot.devices` (`boot_order`); `boot.device`, `boot.changes` (`boot_dirty`) | Info | 57600 (16 hours), both |
| IRQ conflicts | TCP listeners on well known development ports that have been held open a long time | `lsof -nP -iTCP -sTCP:LISTEN -Fpcn`, then `ps -o etime= -p <pid>` for each listener found | `irq_conflict` | `irq.port`, `irq.name`, `irq.pid`, `irq.age` | Warn | 21600 (6 hours) |
| Virus scan | Files that git is tracking in a boot device and that look like secrets or keys | `git --no-optional-locks ls-files -z` | `virus_one`, `virus_many` | `virus.repo`, `virus.file`, `virus.count` | Fail | 86400 (24 hours) |
| Disk trend | How many days until the disk is full, at the rate it has actually been filling over the last fortnight | None; reads the free space already gathered by the fast facts probe | `disk_trend` | `disk.days_left`, `disk.rate` | Warn | 86400 (24 hours) |
| Dotfiles changed | A tracked dotfiles repository with uncommitted work sitting in it | `git --no-optional-locks status --porcelain`, run in the first candidate repository found | `dotfiles_changed` | `dotfiles.repo`, `dotfiles.changes` | Info | 57600 (16 hours) |
| Stale stashes | A git stash in a boot device that is over a month old and never picked back up | `git --no-optional-locks stash list --format=%ct`, run in each boot device in turn | `stale_stashes` | `stash.repo`, `stash.count`, `stash.age` | Info | 57600 (16 hours) |
| Runtime drift | The first boot device pins a node, rust, python or ruby version that is not the one on PATH | `node --version`, `rustc --version`, `python3 --version`, `ruby --version`, whichever the pinning file names | `runtime_drift` | `runtime.repo`, `runtime.name`, `runtime.wanted`, `runtime.found` | Warn | 21600 (6 hours) |
| Battery health | How much of its design capacity the battery still holds, once it first drops below 80 percent and then at each further ten point step | `ioreg -r -c AppleSmartBattery` on macOS; `/sys/class/power_supply/BAT0` or `BAT1` on Linux | `battery_health` | `battery.health`, `battery.cycles` | Info | 86400 (24 hours) |

Boot device order looks under the project roots (see below) for a directory
containing `.git`, at one or two levels deep, and takes the three whose
`.git/index` was modified most recently (falling back to `.git` itself when
`index` does not exist). `boot_order` always fires when at least one device is
found. `boot_dirty` only fires when the single most recent device has at
least one line of `git status --porcelain` output.

IRQ conflicts keeps only listeners on the port allow list below, held by a
process that is not on the system command deny list below, and fires for the
single oldest one that has been listening for at least 8 hours. Only that one
line is shown; other offenders on other ports are not mentioned.

The virus scan checks the same boot devices as the boot device order check,
in the same order, and fires for the first one that has at least one tracked
secret. `virus_one` is used when there is exactly one hit in that repository,
`virus_many` otherwise. `virus.file` is the hit's file name, not its full
path, so the line stays short.

Disk trend keeps up to 14 daily free space readings and needs at least 3 of
them spanning at least a day before it will say anything. A disk that is not
filling, or is filling slowly enough to be more than 30 days off, says
nothing. Otherwise it fires with the days left, rounded up and never below
one, and the rate it is filling at.

Dotfiles changed tries `~/.dotfiles`, `~/dotfiles` and `~/.config`, in that
order, then falls back to `$HOME` itself if that is a repository. The first
one that is a git repository is the one checked; the others are never
looked at.

Stale stashes checks each boot device in turn and fires for the first one
with a stash older than 30 days, reporting the count and the age of the
oldest, worded in days and then in months past 60 days.

Runtime drift looks only at the single most recent boot device, and only at
`.nvmrc`, `rust-toolchain`, `rust-toolchain.toml` and `.tool-versions`, for
node, rust, python and ruby. A pin may be less precise than the version
found (`20` is satisfied by `20.11.0`) but never more precise. It fires on
the first pin that disagrees with what is on PATH; a runtime that is not
installed at all is not a disagreement, since there is nothing to compare
against.

Battery health is deliberately quiet: it speaks once when the battery first
drops below 80 percent of its design capacity, and then only when it crosses
each further ten point step, so a healthy battery, and one that has already
been reported at its current step, says nothing.

## Why `--no-optional-locks`

The git calls above pass `--no-optional-locks`. Without it, git refreshes
the index on disk as a side effect of reading it. Since the boot device order
is ranked by `.git/index` mtime, the probe itself would then be the last
thing to touch that file, pinning whatever repository it had just inspected
at the top of the list forever.

## Port allow list

IRQ conflicts only ever looks at these ports: `1420`, `3000`, `3001`, `4000`,
`4200`, `4321`, `5173`, `5174`, `8000`, `8080`, `8081`, `8888`, `9000`,
`9090`.

## System command deny list

A listener held by one of these commands never counts as an IRQ conflict,
even on an allowed port: `ControlCenter`, `AirPlayXPCHelper`, `rapportd`,
`sharingd`, `remoted`, `identityservicesd`, `mDNSResponder`, `launchd`. macOS
ships these as system services and they hold well known ports as shipped.
They are never a dev server somebody forgot about, so flagging them would
only be noise.

## Secret name patterns

A git-tracked file name is a hit when it is `.env`, or `.env.` followed by
anything; one of `id_rsa`, `id_dsa`, `id_ecdsa`, `id_ed25519`,
`credentials.json`, `.netrc`, `.pgpass`, `secrets.yml`, `secrets.yaml`; a
name starting with `service-account` and ending `.json`; or has an extension
of `pem`, `key`, `p12`, `pfx`, `keystore` or `jks`.

A path is never a hit, regardless of the name, when any of its directory
segments is `test`, `tests`, `fixture`, `fixtures`, `__fixtures__`,
`example`, `examples`, `testdata`, `node_modules`, `vendor` or `target`, or
when the file name ends `.pub`, `.example`, `.sample`, `.template` or
`.dist`, or has an extension of `crt`, `cert` or `cer`. These exclusions are
checked first, so `tests/fixtures/id_rsa` is never a hit even though
`id_rsa` on its own is.

## The cache

The fact cache is one file, `facts.json`, in the cache directory (the usual
`XDG_CACHE_HOME`, or `HOME/.cache`, with a `sparklebios` leaf). It holds
`generated`, the time the probes were last run, and the findings from that
run, plus what a couple of checks carry from one refresh to the next: disk
trend's history of readings, and the health step battery health last spoke
at. The boot path reads the file and does nothing else.

The cache is refreshed once it is older than `REFRESH_AFTER_SECS`, 1800
seconds (30 minutes): past that age, `bios boot` spawns a detached `bios
refresh` after it has already drawn the screen. A finding stops being shown
once it is older than its own `ttl`, independent of the cache's own age.

The consequence is plain: the boot device order and the virus scan result
(ttl 16 hours and 24 hours) are still shown the next morning, but the IRQ
conflicts result (ttl 6 hours) is not, even though the cache itself is only
half an hour stale. And a cold cache, one with no `facts.json` yet, shows no
findings at all on the first boot; that boot's spawned refresh writes the
cache, and the next boot reads it and shows what it found.

## The phrasing table

A finding's `id` is looked up in the current flavour's `[findings]` table
first, when the machine is flavoured and the flavour has that key; otherwise
in the machine's own `[findings]` table. A finding whose `id` has no phrasing
anywhere is not shown. The twelve ids a screen can phrase are `boot_order`,
`boot_dirty`, `irq_conflict`, `virus_one`, `virus_many`, `disk_trend`,
`dotfiles_changed`, `stale_stashes`, `runtime_drift`, `battery_health`, `f1`
and `f1_resume`. The `unicorn` flavour deliberately ships no `[findings]`
table of its own and inherits the machine's wording unchanged.

`f1_resume` is tried first and used whenever it resolves. `f1` is the
fallback, and it appears only when at least one finding is `warn` or `fail`,
so a boot with only `info` findings shows no F1 line at all.

## Config keys

`checks` (default `true`) turns the whole system off when set to `false`: no
cache read, no findings, no refresh spawned.

`project_dirs` (default empty) names the project roots to search. Empty
means the built-in list, whichever of these exist: `~/Code`, `~/code`,
`~/Projects`, `~/projects`, `~/src`, `~/dev`, `~/Developer`, `~/repos`,
`~/work`, `~/git`.

## `bios resume`

`bios resume` prints the absolute path of boot device 1 and exits. It is not
on the boot path, so it runs the project scan itself rather than trusting the
cache, and it never writes the cache. The shell hook wraps the `bios`
function so that typing `bios resume` actually changes directory, since a
plain command cannot change its caller's working directory on its own.
