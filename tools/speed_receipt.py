#!/usr/bin/env python3
"""Times the real boot hook against a release `bios` binary and prints a speed receipt.

Standalone, standard library only: no third-party imports. Runs the same way locally
and in CI.

There are exactly two cases worth a budget, because they are the ones a person or a
script actually hits all day. The first boot of the day plays the animated Full show
on purpose, seconds long, so it is not a speed number:

  1. "A new tab, boot already shown today": every ordinary new tab after the first one.
     `bios boot --hook` decides Fast (src/mode.rs) and draws the static screen at once.
  2. "A shell inside a script, boot skipped": every subshell, and every non-interactive
     script. `SPARKLEBIOS_BOOTED` is already set, so `bios boot --hook` decides Off and
     does nothing at all. This is the floor nearly everything else pays.

A third row, "Starting any program at all, for comparison", runs the exact same harness
against `/usr/bin/true`, so a reader can see how much of the two rows above is bios and
how much is simply the cost of a process existing. It is not checked against a budget.

`bios boot --hook` opens `/dev/tty` directly (src/boot.rs) rather than reading stdout,
so this script runs it against a real pty (the standard library `pty` and `subprocess`
modules): a plain pipe or `/dev/null` is not a tty and the process would never reach the
code being timed. Each run gets its own freshly allocated pty, allocated and closed
outside the timed span, rather than one pty shared across every run: reusing a single
pty as the controlling terminal of a series of separate child processes was tried and is
unreliable (confirmed empirically on macOS: the second child onward fails to claim it,
apparently because the terminal is revoked once the first child's session ends). A fresh
pty per run avoids that, while still keeping the allocation itself out of the number
being measured, which is the point: a real shell already has a terminal, and only pays
for spawning bios.
"""

import argparse
import fcntl
import json
import os
import platform
import pty
import select
import shutil
import statistics
import subprocess
import sys
import tempfile
import termios
import time

# docs/design.md principle 2: "Never slow or break the shell." Section 9's "work before
# first frame" row gives 30ms as the concrete budget for a whole boot. Both rows below
# are held to this single number, because both are timed the same way, the whole
# process, wall clock, start to exit, and design.md's other figure in that table (5ms,
# "off or quiet decision") is a different, in-process measurement this harness, which
# only ever sees a process from the outside, cannot reproduce.
BUDGET_MS = 30.0

RUNS_PER_CASE = 50
WARMUP_RUNS = 5
KEPT_RUNS = RUNS_PER_CASE - WARMUP_RUNS

# A literal always present in machines/pc95.toml's "Main Processor : {cpu.name}, ..."
# step: not flavoured, not conditional on findings, so it is in every Fast render.
KNOWN_SCREEN_TEXT = b"Main Processor :"
MIN_SCREEN_BYTES = 100

# A short, fixed poll interval used only while a child is still running, so its writes
# are drained as they arrive and it can never block on a full pty output buffer. Once
# the child has exited, draining continues with a zero timeout until nothing is left.
DRAIN_POLL_SECS = 0.0005

FAST_LABEL = "A new tab, boot already shown today"
SKIPPED_LABEL = "A shell inside a script, boot skipped"
BASELINE_LABEL = "Starting any program at all, for comparison"
BASELINE_PROGRAM = "/usr/bin/true"


def fail(message):
    print(f"speed_receipt: {message}", file=sys.stderr)
    sys.exit(1)


def build_sandbox():
    """A temp sandbox with its own HOME and all four XDG directories, so this never
    reads or writes the caller's real config, state or cache."""
    root = tempfile.mkdtemp(prefix="sparklebios-speed-")
    dirs = {name: os.path.join(root, name) for name in ("home", "config", "state", "cache", "data")}
    for path in dirs.values():
        os.makedirs(path, exist_ok=True)
    state_dir = os.path.join(dirs["state"], "sparklebios")
    cache_dir = os.path.join(dirs["cache"], "sparklebios")
    os.makedirs(state_dir, exist_ok=True)
    os.makedirs(cache_dir, exist_ok=True)
    return root, dirs, state_dir, cache_dir


def base_env(dirs):
    """Built from scratch rather than inherited: a locally set TERM_PROGRAM, NO_COLOR or
    SPARKLEBIOS_* would change what gets measured and make a local run disagree with CI."""
    return {
        "HOME": dirs["home"],
        "XDG_CONFIG_HOME": dirs["config"],
        "XDG_STATE_HOME": dirs["state"],
        "XDG_CACHE_HOME": dirs["cache"],
        "XDG_DATA_HOME": dirs["data"],
        "TERM": "xterm-256color",
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
    }


def seed_warm_cache(cache_dir, now):
    """Writes `facts.json` directly rather than running `bios refresh`: the sandboxed
    HOME has no repos, open ports or secrets to probe anyway, so a real refresh would
    find nothing more than this does, and writing it directly is deterministic. A fresh
    `generated` keeps the cache inside its staleness window (cache.rs) for the whole run,
    so `bios boot --hook` never spawns a detached refresh mid-measurement."""
    cache = {"generated": now, "findings": [], "disk_history": [], "battery_step": None}
    with open(os.path.join(cache_dir, "facts.json"), "w") as f:
        json.dump(cache, f)


def seed_fast_state(state_dir, now, today):
    """`bios boot --hook` rewrites state.json's `last_boot` to "now" after every call that
    is not Off (src/boot.rs), so this has to run again before every single Fast-mode
    call: otherwise the next call falls inside the 10 second burst window (src/mode.rs)
    and decides Quiet instead of Fast. `last_full_day` set to today is what keeps the
    decision Fast rather than Full."""
    state = {
        "last_boot": now - 3600,
        "last_full_day": today,
        "streak_days": 0,
        "streak_last_day": None,
    }
    with open(os.path.join(state_dir, "state.json"), "w") as f:
        json.dump(state, f)


def _preexec_controlling_tty():
    """Runs in the child, after fork, before exec, with fd 0/1/2 already the pty slave
    (subprocess sets those up before calling preexec_fn): makes the child a session
    leader and gives it that pty as its controlling terminal, which is what lets
    `bios boot --hook` open /dev/tty at all."""
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)


def run_and_drain(master, slave, argv, env):
    """Runs `argv` against the given pty pair and returns every byte it wrote. Drains
    concurrently while the child runs, not only after it exits, so a child can never
    block on a full pty output buffer."""
    proc = subprocess.Popen(
        argv,
        stdin=slave,
        stdout=slave,
        stderr=slave,
        env=env,
        preexec_fn=_preexec_controlling_tty,
        close_fds=True,
    )
    os.close(slave)  # the child has its own reference to it; this process does not need one
    captured = bytearray()
    while True:
        exited = proc.poll() is not None
        ready, _, _ = select.select([master], [], [], 0 if exited else DRAIN_POLL_SECS)
        if ready:
            try:
                chunk = os.read(master, 65536)
            except OSError:
                chunk = b""
            if chunk:
                captured += chunk
                continue
        if exited:
            break
    return bytes(captured)


def run_hook_timed(argv, env):
    """Allocates a fresh pty, times only the run against it, and returns
    (elapsed_ms, captured_bytes). Allocating and closing the pty happen outside the
    timed span: a real shell already has a terminal, so only the spawn itself, not the
    cost of making a terminal exist, belongs in the number."""
    master, slave = pty.openpty()
    start = time.perf_counter()
    out = run_and_drain(master, slave, argv, env)
    elapsed_ms = (time.perf_counter() - start) * 1000.0
    os.close(master)
    return elapsed_ms, out


def check_cases_do_what_they_claim(bios_path, fast_env, off_env, state_dir, now, today):
    """A receipt that measures nothing is worse than no receipt: before timing anything,
    prove each case actually did what its label claims. Runs once, not inside the timing
    loop."""
    hook_argv = [bios_path, "boot", "--hook"]

    seed_fast_state(state_dir, now, today)
    _, out = run_hook_timed(hook_argv, fast_env)
    if len(out) < MIN_SCREEN_BYTES or KNOWN_SCREEN_TEXT not in out:
        fail(
            f"'{FAST_LABEL}' did not draw a boot screen: got {len(out)} byte(s), "
            f"expected at least {MIN_SCREEN_BYTES} including {KNOWN_SCREEN_TEXT!r}. "
            "The Fast-mode seed in state.json, or the pty's controlling-terminal setup, "
            "is not producing the Fast decision."
        )

    _, out = run_hook_timed(hook_argv, off_env)
    if len(out) != 0:
        fail(
            f"'{SKIPPED_LABEL}' wrote {len(out)} byte(s), expected zero. "
            "SPARKLEBIOS_BOOTED is not producing the Off decision."
        )


def time_case(argv, env, before_each=None):
    timings = []
    for _ in range(RUNS_PER_CASE):
        if before_each:
            before_each()
        elapsed_ms, _ = run_hook_timed(argv, env)
        timings.append(elapsed_ms)
    return timings[WARMUP_RUNS:]


def median_and_slowest(timings):
    return statistics.median(timings), max(timings)


def render_table(rows, runner):
    lines = [
        "### Speed receipt",
        "",
        f"| What | Median | Slowest of {KEPT_RUNS} |",
        "| --- | --- | --- |",
    ]
    for label, median, slowest in rows:
        lines.append(f"| {label} | {median:.1f}ms | {slowest:.1f}ms |")
    lines.append("")
    lines.append(
        f"Budget is {BUDGET_MS:.0f}ms for both, from docs/design.md. Measured on {runner}, "
        f"{RUNS_PER_CASE} runs each with the first {WARMUP_RUNS} discarded."
    )
    lines.append(
        "The comparison row is not checked against a budget: it just shows what a bare "
        "process spawn costs on its own."
    )
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bios_path", help="Path to a release bios binary.")
    parser.add_argument(
        "--summary", help="Append the Markdown table to this file instead of printing it."
    )
    parser.add_argument(
        "--runner",
        default=platform.system() or "unknown",
        help="Runner name for the table footer, e.g. macos-latest.",
    )
    args = parser.parse_args()

    bios_path = os.path.abspath(args.bios_path)
    if not os.path.isfile(bios_path) or not os.access(bios_path, os.X_OK):
        fail(f"no executable bios binary at {bios_path}")
    if not os.path.isfile(BASELINE_PROGRAM):
        fail(f"no {BASELINE_PROGRAM} to use for the comparison row")

    root, dirs, state_dir, cache_dir = build_sandbox()
    try:
        now = int(time.time())
        today = time.strftime("%Y-%m-%d", time.localtime(now))
        seed_warm_cache(cache_dir, now)

        env = base_env(dirs)
        fast_env = dict(env)
        fast_env.pop("SPARKLEBIOS_BOOTED", None)
        off_env = dict(env)
        off_env["SPARKLEBIOS_BOOTED"] = "1"

        check_cases_do_what_they_claim(bios_path, fast_env, off_env, state_dir, now, today)

        hook_argv = [bios_path, "boot", "--hook"]
        fast_timings = time_case(
            hook_argv,
            fast_env,
            before_each=lambda: seed_fast_state(state_dir, now, today),
        )
        skipped_timings = time_case(hook_argv, off_env)
        baseline_timings = time_case([BASELINE_PROGRAM], env)

        fast_median, fast_slowest = median_and_slowest(fast_timings)
        skipped_median, skipped_slowest = median_and_slowest(skipped_timings)
        baseline_median, baseline_slowest = median_and_slowest(baseline_timings)

        table = render_table(
            [
                (FAST_LABEL, fast_median, fast_slowest),
                (SKIPPED_LABEL, skipped_median, skipped_slowest),
                (BASELINE_LABEL, baseline_median, baseline_slowest),
            ],
            args.runner,
        )

        if args.summary:
            with open(args.summary, "a") as f:
                f.write(table)
        else:
            print(table, end="")

        overs = []
        if fast_median > BUDGET_MS:
            overs.append(
                f"'{FAST_LABEL}' median {fast_median:.1f}ms is over the "
                f"{BUDGET_MS:.0f}ms budget by {fast_median - BUDGET_MS:.1f}ms"
            )
        if skipped_median > BUDGET_MS:
            overs.append(
                f"'{SKIPPED_LABEL}' median {skipped_median:.1f}ms is over the "
                f"{BUDGET_MS:.0f}ms budget by {skipped_median - BUDGET_MS:.1f}ms"
            )
        if overs:
            for message in overs:
                print(f"speed_receipt: {message}", file=sys.stderr)
            sys.exit(1)
    finally:
        shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    main()
