//! Live pty tests for the ULTRA sprinkles hook and the presence feature it sits on top of (see
//! `docs/sprinkles.md`'s "ultra" section and `src/shell.rs`).
//!
//! Every check below was verified by hand on a real pty before it became a test, and each one
//! needs a real interactive shell to catch: the regressions here (a bash `DEBUG` trap silently
//! reverted, bash mistaking its own `PROMPT_COMMAND` chain for a typed command, and bash's title
//! feature clobbering the exit status presence relies on) only ever show up once a real shell
//! runs a real command on a real prompt loop. Rendering a hook and parsing it, or running it
//! once and reading the first line, catches none of them.
//!
//! Check 5 ("below Ultra, a hook never mentions ultra") is not duplicated here: it is already
//! pinned by `a_hook_below_ultra_never_mentions_ultra_in_any_shell` in `src/shell.rs`.
//!
//! All the `forkpty` plumbing lives in `Session`, below, so no test touches an unsafe block
//! directly. Every shell it spawns is sandboxed with its own throwaway `HOME`/XDG directories
//! (see `sandbox_env`), never the developer's own config or state, and every wait is bounded by
//! a deadline so a hung or broken shell fails in seconds rather than hanging the suite. Reaping
//! never calls a blocking `waitpid`: it polls `WNOHANG` against a deadline and only escalates to
//! `SIGTERM` then `SIGKILL` if the shell will not go quietly.

use std::ffi::CString;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sparklebios::config::Config;
use sparklebios::flavour::Flavour;
use sparklebios::shell::{render_hook, Mascot, Shell};
use sparklebios::sprinkles::Level;

/// How long a session is given to run its commands, say goodbye and exit before a test fails
/// outright. Generous enough for a slow CI runner and for the two checks that have to run a
/// real `sleep` to clear `presence_after`, short enough that a genuinely hung shell does not
/// stall the suite.
const DEADLINE: Duration = Duration::from_secs(8);

/// The three shells this project ships a hook for, and the name each one is looked up under.
const SHELLS: [(Shell, &str); 3] = [
    (Shell::Zsh, "zsh"),
    (Shell::Bash, "bash"),
    (Shell::Fish, "fish"),
];

/// Ultra, with a chance of zero so the random folder remarks and reactions never fire (they
/// would otherwise make the captured bytes flaky) and `presence_after` at zero so the presence
/// finish line fires on every command instead of only ones that ran for a while, keeping every
/// session here short. Title and presence are left at their real defaults (both on), because
/// check 2 is specifically about the default configuration.
fn ultra_config() -> Config {
    Config {
        sprinkles: Level::Ultra,
        presence_after: 0,
        ultra_chance: 0,
        ..Config::default()
    }
}

/// Like `ultra_config`, but with a real `presence_after` threshold. Sourcing the rc file itself
/// runs a few ordinary commands after the `DEBUG` trap is already armed (the last `eval` in the
/// hook, `command -v` checks, and so on), each of which arms presence's own idle timer; at
/// `presence_after = 0` that spurious, near-instant timer clears the threshold on its own and
/// prints an extra, unearned finish line before a test ever types anything, which corrupts an
/// exact count. Used only by the two checks that count occurrences of the finish line's own
/// wording; paired with a command that actually runs long enough to clear this threshold on
/// purpose (`sleep`), rather than one that finishes near-instantly like the spurious one does.
fn presence_config() -> Config {
    Config {
        presence_after: 1,
        ..ultra_config()
    }
}

fn ninja() -> Flavour {
    sparklebios::flavour::find("ninja", None).unwrap()
}

/// Absolute path to a shell on `$PATH`, or `None` if it is not installed. Mirrors the
/// skip-cleanly spirit of `tests/cli.rs`'s `parses`: a machine without fish still passes.
fn find_shell(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

/// The directory the `bios` binary this test run just built lives in, so a sandboxed shell's
/// `$PATH` can find it without ever touching the developer's real `$PATH`.
fn bios_bin_dir() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_bios"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// The full, explicit environment for a sandboxed shell: a throwaway `HOME` and the four XDG
/// directories under it (never the real ones), `SPARKLEBIOS_BOOT=0` so the boot show itself
/// never runs and pollutes the captured bytes, and nothing inherited from the ambient
/// environment. Built fresh, in the same call that spawns the shell, per the sandboxing rule.
///
/// `TERM` is chosen per shell rather than one value for all three: fish (as of the version this
/// was written against) probes the terminal for colour and OS-name support with escape sequence
/// queries whenever `$TERM` looks like it might support them, and hangs waiting for a reply that
/// nothing on the other end of this pty is ever going to send; `dumb` is the one value that
/// makes it skip the probe. zsh and bash need the opposite: with `TERM=dumb` zsh treats the
/// shell as having no usable terminal at all and never populates `$LINES`/`$COLUMNS`, which
/// then trips `nounset` inside the rain animation's own bare `$COLUMNS` reference at exit, a
/// failure that has nothing to do with what this suite is trying to pin.
fn sandbox_env(sandbox: &Path, shell: Shell) -> Vec<(String, String)> {
    let path = format!(
        "{}:/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin:/usr/local/bin",
        bios_bin_dir().display()
    );
    let term = match shell {
        Shell::Fish => "dumb",
        Shell::Zsh | Shell::Bash => "xterm-256color",
    };
    vec![
        ("HOME".into(), sandbox.display().to_string()),
        (
            "XDG_CONFIG_HOME".into(),
            sandbox.join("config").display().to_string(),
        ),
        (
            "XDG_STATE_HOME".into(),
            sandbox.join("state").display().to_string(),
        ),
        (
            "XDG_CACHE_HOME".into(),
            sandbox.join("cache").display().to_string(),
        ),
        (
            "XDG_DATA_HOME".into(),
            sandbox.join("data").display().to_string(),
        ),
        ("PATH".into(), path),
        ("TERM".into(), term.into()),
        ("LANG".into(), "C.UTF-8".into()),
        ("SPARKLEBIOS_BOOT".into(), "0".into()),
    ]
}

fn shell_name(shell: Shell) -> &'static str {
    match shell {
        Shell::Zsh => "zsh",
        Shell::Bash => "bash",
        Shell::Fish => "fish",
    }
}

/// Renders the hook for `shell` and installs it as that shell's own startup file under
/// `sandbox` (`~/.zshrc`, `~/.bashrc`, or `$XDG_CONFIG_HOME/fish/config.fish`), with `prelude`
/// written ahead of it verbatim. Also writes the same rendered hook to a standalone file
/// alongside it, and returns that file's path, for a test that needs to `source` it a second
/// time. `prelude` is where a nounset test turns strict mode on before the hook it is about to
/// exercise first runs.
fn install_hook(
    sandbox: &Path,
    shell: Shell,
    cfg: &Config,
    flavour: &Flavour,
    prelude: &str,
) -> PathBuf {
    let hook = render_hook(
        shell,
        cfg,
        Some(flavour),
        Mascot::none(),
        false,
        "/nonexistent/sparklebios-sounds",
    );
    let standalone = sandbox.join(format!("hook.{}", shell_name(shell)));
    std::fs::write(&standalone, &hook).unwrap();

    let rc_path = match shell {
        Shell::Zsh => sandbox.join(".zshrc"),
        Shell::Bash => sandbox.join(".bashrc"),
        Shell::Fish => {
            let dir = sandbox.join("config").join("fish");
            std::fs::create_dir_all(&dir).unwrap();
            dir.join("config.fish")
        }
    };
    std::fs::write(&rc_path, format!("{prelude}{hook}")).unwrap();
    standalone
}

/// A live shell on a pty. All the `forkpty` plumbing the tests below need lives here, so no
/// test has to touch an unsafe block itself. Owns the sandbox its hook was installed under, so
/// the sandbox lives exactly as long as the session that might still need to read files out of
/// it (an explicit `source` of a second copy of the hook, for instance), and is cleaned up the
/// moment the session is, rather than leaking a directory per test.
struct Session {
    master: std::fs::File,
    pid: libc::pid_t,
    out: Vec<u8>,
    reaped: bool,
    _sandbox: tempfile::TempDir,
}

impl Session {
    /// Forks a pty child that `execve`s `program` with `args` and exactly `env`, nothing
    /// inherited, on an 80x24 window. Every `CString` and pointer is built before the fork, so
    /// the child does nothing between `forkpty` returning and the `execve` call itself: this is
    /// what keeps forking safe to do from a multi-threaded test binary.
    fn spawn(
        program: &Path,
        args: &[&str],
        env: &[(String, String)],
        sandbox: tempfile::TempDir,
    ) -> Session {
        let program_str = program.to_str().expect("a utf8 shell path");
        let prog_c = CString::new(program_str).unwrap();
        let argv_c: Vec<CString> = std::iter::once(program_str.to_string())
            .chain(args.iter().map(|s| s.to_string()))
            .map(|s| CString::new(s).unwrap())
            .collect();
        let mut argv_ptrs: Vec<*const libc::c_char> = argv_c.iter().map(|c| c.as_ptr()).collect();
        argv_ptrs.push(std::ptr::null());

        let envp_c: Vec<CString> = env
            .iter()
            .map(|(k, v)| CString::new(format!("{k}={v}")).unwrap())
            .collect();
        let mut envp_ptrs: Vec<*const libc::c_char> = envp_c.iter().map(|c| c.as_ptr()).collect();
        envp_ptrs.push(std::ptr::null());

        let mut master: libc::c_int = -1;
        let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
        ws.ws_row = 24;
        ws.ws_col = 80;

        // Safety: everything the child could need (the C strings and the pointer arrays above)
        // was built in the parent before this call. Between `forkpty` returning zero and the
        // `execve` below, the child touches nothing but the syscall itself.
        let pid = unsafe {
            libc::forkpty(
                &mut master,
                std::ptr::null_mut::<libc::c_char>(),
                std::ptr::null_mut::<libc::termios>(),
                // A raw pointer, not `&mut ws`: this argument is `*mut winsize` on macOS and
                // `*const winsize` on Linux. A mutable reference satisfies both by coercion but
                // clippy calls it an unnecessary `mut` on Linux, so name the pointer instead.
                std::ptr::addr_of_mut!(ws),
            )
        };
        assert!(
            pid >= 0,
            "forkpty failed: {}",
            std::io::Error::last_os_error()
        );
        if pid == 0 {
            unsafe {
                libc::execve(prog_c.as_ptr(), argv_ptrs.as_ptr(), envp_ptrs.as_ptr());
                libc::_exit(127);
            }
        }
        let master = unsafe { std::fs::File::from_raw_fd(master) };
        // The pty's own line discipline echoes back whatever is written to the master by
        // default, which is only noise here (nothing is asserted against a typed command
        // being echoed back). A shell with its own line editor, which is every shell this
        // suite drives, does its own echo once it starts reading a line, independently of
        // this setting, so turning the kernel's off costs it nothing.
        unsafe {
            let mut term: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(master.as_raw_fd(), &mut term) == 0 {
                term.c_lflag &= !(libc::ECHO as libc::tcflag_t);
                libc::tcsetattr(master.as_raw_fd(), libc::TCSANOW, &term);
            }
        }
        Session {
            master,
            pid,
            out: Vec::new(),
            reaped: false,
            _sandbox: sandbox,
        }
    }

    /// Types `line` followed by Enter, as a user would. The pty's own input queue holds it
    /// until the shell is ready to read it, so nothing here has to wait for the previous
    /// command's prompt before sending the next one. A write failure (the shell has already
    /// exited) is not this method's problem to report.
    fn send(&mut self, line: &str) {
        let _ = self.master.write_all(line.as_bytes());
        let _ = self.master.write_all(b"\n");
    }

    /// Tells the shell to exit, reads everything it writes on the way out (bounded by
    /// `deadline`), reaps it, and returns everything captured over the whole session as text.
    fn run_to_exit(mut self, deadline: Duration) -> String {
        self.send("exit");
        self.read_until_eof(deadline);
        self.reap(Duration::from_secs(2));
        String::from_utf8_lossy(&self.out).into_owned()
    }

    /// Reads until the pty closes (the shell and anything it spawned are gone) or `deadline`,
    /// whichever comes first, appending everything read to `self.out`.
    fn read_until_eof(&mut self, deadline: Duration) {
        let end = Instant::now() + deadline;
        loop {
            let remaining = end.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return;
            }
            let mut pfd = libc::pollfd {
                fd: self.master.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let step = remaining.min(Duration::from_millis(200));
            let n = unsafe { libc::poll(&mut pfd, 1, step.as_millis() as libc::c_int) };
            if n <= 0 {
                continue; // nothing ready in this slice; loop back and re-check the deadline
            }
            let mut chunk = [0u8; 4096];
            match self.master.read(&mut chunk) {
                Ok(0) => return,
                Ok(n) => self.out.extend_from_slice(&chunk[..n]),
                Err(e) if e.raw_os_error() == Some(libc::EIO) => return,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("pty read error: {e}"),
            }
        }
    }

    /// `WNOHANG`, polled against a deadline, never a blocking `waitpid`: escalates to `SIGTERM`
    /// then `SIGKILL` only if the shell will not exit on its own.
    fn reap(&mut self, deadline: Duration) {
        if self.wait_exited(deadline) {
            return;
        }
        unsafe { libc::kill(self.pid, libc::SIGTERM) };
        if self.wait_exited(Duration::from_secs(1)) {
            return;
        }
        unsafe { libc::kill(self.pid, libc::SIGKILL) };
        self.wait_exited(Duration::from_secs(1));
    }

    fn wait_exited(&mut self, deadline: Duration) -> bool {
        if self.reaped {
            return true;
        }
        let end = Instant::now() + deadline;
        loop {
            let mut status: libc::c_int = 0;
            let r = unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) };
            if r != 0 {
                // Either we reaped it (`r == self.pid`) or it is already gone (`r < 0`,
                // `ECHILD`): either way there is nothing left here to wait for.
                self.reaped = true;
                return true;
            }
            if Instant::now() >= end {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Best effort only: a test that panics before `run_to_exit` must not leak a shell that
        // outlives the test binary.
        if !self.reaped {
            unsafe { libc::kill(self.pid, libc::SIGKILL) };
            self.wait_exited(Duration::from_secs(1));
        }
    }
}

/// Spawns `shell` interactively (`-i`) over a fresh sandbox: its own hook already installed as
/// that shell's own startup file. `None` when the shell is not installed, so a caller can skip
/// cleanly.
fn interactive_session(shell: Shell, cfg: &Config, prelude: &str) -> Option<(Session, PathBuf)> {
    let name = shell_name(shell);
    let path = find_shell(name)?;
    let sandbox = tempfile::tempdir().unwrap();
    let standalone = install_hook(sandbox.path(), shell, cfg, &ninja(), prelude);
    let env = sandbox_env(sandbox.path(), shell);
    let session = Session::spawn(&path, &["-i"], &env, sandbox);
    Some((session, standalone))
}

/// Check 1: `SPARKLEBIOS_STATUS` is `ERR 0x01` after a failing command and empty after a
/// successful one, at Ultra, in all three shells.
#[test]
fn sparklebios_status_reports_the_last_commands_exit_code() {
    let status_probe = r#"printf 'STATUS[%s]\n' "$SPARKLEBIOS_STATUS""#;
    for (shell, name) in SHELLS {
        let Some((mut session, _)) = interactive_session(shell, &ultra_config(), "") else {
            continue;
        };
        session.send("false");
        session.send(status_probe);
        session.send("true");
        session.send(status_probe);
        let out = session.run_to_exit(DEADLINE);
        assert!(
            out.contains("STATUS[ERR 0x01]"),
            "{name} never reported ERR 0x01 after a failing command:\n{out}"
        );
        assert!(
            out.contains("STATUS[]"),
            "{name} did not clear SPARKLEBIOS_STATUS after a successful command:\n{out}"
        );
    }
}

/// Check 2, the successful half: a command that succeeds says "done in" and never
/// "failed after", across more than one prompt (the two known bash bugs here only ever broke
/// things starting on the second prompt, so a single command is not enough to catch a
/// regression of that shape).
#[test]
fn presence_reports_a_successful_command_as_done_never_failed() {
    for (shell, name) in SHELLS {
        let Some((mut session, _)) = interactive_session(shell, &presence_config(), "") else {
            continue;
        };
        session.send("sleep 1.2");
        session.send("sleep 1.2");
        let out = session.run_to_exit(DEADLINE);
        assert_eq!(
            out.matches("Ninja: done in").count(),
            2,
            "{name} did not report both successful commands as done:\n{out}"
        );
        assert!(
            !out.contains("Ninja: failed after"),
            "{name} reported a successful command as failed:\n{out}"
        );
        assert_eq!(
            out.matches("Ninja: you never saw me.").count(),
            1,
            "{name}'s goodbye did not appear exactly once:\n{out}"
        );
    }
}

/// Check 2, the failing half: a command that fails says "failed after" and never "done in",
/// again across more than one prompt. In bash, with the default configuration (title on), this
/// is currently red: bash's title feature runs ahead of presence's own finish-line function in
/// `PROMPT_COMMAND` and clobbers `$?` with its own (always successful) exit status before
/// presence ever reads it, so a failing command is misreported as done. Tracked as a real,
/// already-shipped bug; zsh and fish are unaffected because each of their prompt hooks gets the
/// real exit status.
#[test]
fn presence_reports_a_failing_command_as_failed_never_done() {
    for (shell, name) in SHELLS {
        let Some((mut session, _)) = interactive_session(shell, &presence_config(), "") else {
            continue;
        };
        session.send("sleep 1.2; false");
        session.send("sleep 1.2; false");
        let out = session.run_to_exit(DEADLINE);
        assert_eq!(
            out.matches("Ninja: failed after").count(),
            2,
            "{name} did not report both failing commands as failed:\n{out}"
        );
        assert!(
            !out.contains("Ninja: done in"),
            "{name} reported a failing command as done:\n{out}"
        );
        assert_eq!(
            out.matches("Ninja: you never saw me.").count(),
            1,
            "{name}'s goodbye did not appear exactly once:\n{out}"
        );
    }
}

/// Check 3, shared by the zsh and bash tests below: sourcing the hook a second time in the same
/// shell stays silent (no doubled goodbye, no corrupted state) under strict unset checking, with
/// real commands run afterwards so strict mode actually has code exercising the variables it
/// would trip on. A hook rendered and parsed in isolation cannot catch a bare unset-variable
/// reference: it only ever shows up once a real shell with strict mode on actually reaches that
/// line. Kept as one function per shell, rather than a loop over both, so a failure pinning one
/// does not stop the other from being checked in the same run.
fn hook_sourced_twice_stays_silent_under_strict_unset_checking(shell: Shell, prelude: &str) {
    let name = shell_name(shell);
    let Some((mut session, standalone)) = interactive_session(shell, &ultra_config(), prelude)
    else {
        return;
    };
    session.send(&format!("source '{}'", standalone.display()));
    session.send("false");
    session.send(r#"printf 'STATUS[%s]\n' "$SPARKLEBIOS_STATUS""#);
    session.send("true");
    let out = session.run_to_exit(DEADLINE);
    let lower = out.to_lowercase();
    assert!(
        !lower.contains("unbound variable") && !lower.contains("parameter not set"),
        "{name} tripped strict unset checking after sourcing the hook twice:\n{out}"
    );
    assert!(
        out.contains("STATUS[ERR 0x01]"),
        "{name} lost track of the exit code after sourcing the hook twice:\n{out}"
    );
    assert_eq!(
        out.matches("Ninja: you never saw me.").count(),
        1,
        "{name} said goodbye more than once, so something doubled up:\n{out}"
    );
}

#[test]
fn hook_sourced_twice_stays_silent_under_setopt_nounset_in_zsh() {
    hook_sourced_twice_stays_silent_under_strict_unset_checking(Shell::Zsh, "setopt nounset\n");
}

#[test]
fn hook_sourced_twice_stays_silent_under_set_dash_u_in_bash() {
    hook_sourced_twice_stays_silent_under_strict_unset_checking(Shell::Bash, "set -u\n");
}

/// Check 4, the SSH half: over `SSH_CONNECTION`, the ultra block never runs at all, not even far
/// enough to export `SPARKLEBIOS_STATUS`. `presence` and `title` are not gated on
/// `SSH_CONNECTION` (only the ultra-specific block is), so this checks the variable the ultra
/// block itself owns rather than the whole hook's output.
#[test]
fn ultra_never_initialises_over_an_ssh_connection() {
    for (shell, name) in SHELLS {
        let Some(path) = find_shell(name) else {
            continue;
        };
        let sandbox = tempfile::tempdir().unwrap();
        install_hook(sandbox.path(), shell, &ultra_config(), &ninja(), "");
        let mut env = sandbox_env(sandbox.path(), shell);
        env.push((
            "SSH_CONNECTION".into(),
            "203.0.113.5 51413 203.0.113.9 22".into(),
        ));
        let mut session = Session::spawn(&path, &["-i"], &env, sandbox);
        let probe = match shell {
            Shell::Fish => {
                "if set -q SPARKLEBIOS_STATUS; printf 'SSH_STATUS=SET\\n'; else; printf 'SSH_STATUS=UNSET\\n'; end"
            }
            _ => {
                r#"if [ -z "${SPARKLEBIOS_STATUS+x}" ]; then printf 'SSH_STATUS=UNSET\n'; else printf 'SSH_STATUS=SET\n'; fi"#
            }
        };
        session.send(probe);
        let out = session.run_to_exit(DEADLINE);
        assert!(
            out.contains("SSH_STATUS=UNSET"),
            "{name} let ultra initialise over SSH_CONNECTION:\n{out}"
        );
    }
}

/// Check 4, the non-interactive half: the whole hook, ultra included, does nothing at all when
/// the shell it is sourced into is not interactive. Run with `-c`, which every one of the three
/// shells treats as non-interactive regardless of whether a pty is attached, so
/// `status is-interactive` / `[[ -o interactive ]]` / `[[ $- == *i* ]]` all read false and the
/// hook's own top-level guard should stop it before it does anything, fish's own always-sourced
/// `config.fish` included.
#[test]
fn the_hook_does_nothing_in_a_noninteractive_shell() {
    for (shell, name) in SHELLS {
        let Some(path) = find_shell(name) else {
            continue;
        };
        let sandbox = tempfile::tempdir().unwrap();
        let standalone = install_hook(sandbox.path(), shell, &ultra_config(), &ninja(), "");
        let env = sandbox_env(sandbox.path(), shell);
        let command = format!("source '{}'; printf '%s' MARKER_DONE", standalone.display());
        let session = Session::spawn(&path, &["-c", &command], &env, sandbox);
        let out = session.run_to_exit(DEADLINE);
        assert_eq!(
            out, "MARKER_DONE",
            "{name} produced output from a non-interactive source:\n{out}"
        );
    }
}
