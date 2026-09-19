//! Raw terminal mode and key polling, for the animated show's skip key.

/// Puts the terminal on `fd` into non-canonical, no-echo mode (`ICANON`, `ECHO` and `ISIG` off,
/// `VMIN=0`, `VTIME=0`) and restores the saved termios on drop. `None` when `fd` is not a
/// terminal, or the mode change fails; either way the caller is left with the terminal exactly
/// as it found it.
pub struct RawGuard {
    fd: i32,
    saved: libc::termios,
}

impl RawGuard {
    pub fn new(fd: i32) -> Option<RawGuard> {
        // SAFETY: `fd` is the caller's file descriptor; `isatty` only reads it.
        if unsafe { libc::isatty(fd) } == 0 {
            return None;
        }
        // SAFETY: `termios` is a plain C struct of integer and array fields, so an all zero bit
        // pattern is valid; it is only read after `tcgetattr` has had a chance to fill it in.
        let mut saved: libc::termios = unsafe { std::mem::zeroed() };
        // SAFETY: `fd` is a terminal (checked above) and `saved` points at a valid, writable
        // `termios` we own for the duration of the call.
        if unsafe { libc::tcgetattr(fd, &mut saved) } != 0 {
            return None;
        }
        let mut raw = saved;
        raw.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
        raw.c_cc[libc::VMIN] = 0;
        raw.c_cc[libc::VTIME] = 0;
        // SAFETY: `fd` is a terminal and `raw` is a valid `termios` built from one just read
        // from that same fd.
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) } != 0 {
            return None;
        }
        Some(RawGuard { fd, saved })
    }
}

impl Drop for RawGuard {
    fn drop(&mut self) {
        // SAFETY: `self.fd` was a terminal when this guard was created and `self.saved` is the
        // termios read from it then; restoring it is safe to attempt even if the fd's state has
        // since changed, and any failure here is silently ignored, which is the whole point of
        // this guard: the terminal must never be left in raw mode.
        unsafe {
            libc::tcsetattr(self.fd, libc::TCSANOW, &self.saved);
        }
    }
}

/// The longest a single wait for readiness is allowed to block: keeps a caller's key-skip check
/// responsive even mid-wait, and bounds how late `wait_for_key(fd, 0)` can return.
const POLL_SLICE_MS: u64 = 50;

/// Waits up to `ms` for input on `fd`, in slices of at most `POLL_SLICE_MS` so no single wait
/// ever blocks longer than that. Returns the bytes read, empty on timeout or error.
///
/// Uses `select`, not `poll`: a `poll` on an fd opened by reading `/dev/tty` directly (rather
/// than the underlying pty device) reports `POLLNVAL` on macOS even though the fd is perfectly
/// readable, which is exactly the fd `--hook` mode polls. `select` does not have this problem.
pub fn wait_for_key(fd: i32, ms: u64) -> Vec<u8> {
    let start = std::time::Instant::now();
    let total = std::time::Duration::from_millis(ms);
    loop {
        let elapsed = start.elapsed();
        if elapsed >= total {
            return Vec::new();
        }
        let remaining_ms = (total - elapsed).as_millis() as u64;
        let slice_ms = remaining_ms.min(POLL_SLICE_MS);
        let mut read_fds: libc::fd_set = unsafe { std::mem::zeroed() };
        // SAFETY: `read_fds` is a valid, writable `fd_set` on the stack, and `fd` is the
        // caller's file descriptor; `FD_ZERO` and `FD_SET` only write into `read_fds`.
        unsafe {
            libc::FD_ZERO(&mut read_fds);
            libc::FD_SET(fd, &mut read_fds);
        }
        let mut timeout = libc::timeval {
            tv_sec: (slice_ms / 1000) as libc::time_t,
            tv_usec: ((slice_ms % 1000) * 1000) as libc::suseconds_t,
        };
        // SAFETY: `read_fds` and `timeout` are valid, writable values this call owns for its
        // duration; the write and error fd sets are null, which `select` treats as empty.
        let rc = unsafe {
            libc::select(
                fd + 1,
                &mut read_fds,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut timeout,
            )
        };
        if rc < 0 {
            return Vec::new();
        }
        // SAFETY: `fd` and `read_fds` are the same values just passed to `select` above.
        if rc > 0 && unsafe { libc::FD_ISSET(fd, &read_fds) } {
            let mut buf = [0u8; 256];
            // SAFETY: `fd` is readable per `select` above, and `buf` is a valid, writable
            // buffer of the length passed.
            let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if n > 0 {
                return buf[..n as usize].to_vec();
            }
            return Vec::new();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::io::AsRawFd;

    #[test]
    fn raw_guard_is_none_on_a_non_terminal_fd() {
        let file = tempfile::tempfile().unwrap();
        assert!(RawGuard::new(file.as_raw_fd()).is_none());
    }

    fn make_pipe() -> (i32, i32) {
        let mut fds = [0i32; 2];
        // SAFETY: `fds` is a valid, writable array of two `c_int`s.
        let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
        assert_eq!(rc, 0, "libc::pipe failed");
        (fds[0], fds[1])
    }

    #[test]
    fn wait_for_key_returns_data_already_on_a_pipe() {
        let (read_fd, write_fd) = make_pipe();
        // SAFETY: `write_fd` is a valid, open, writable fd from the pipe just created above.
        let n = unsafe { libc::write(write_fd, b"x".as_ptr() as *const libc::c_void, 1) };
        assert_eq!(n, 1);
        let bytes = wait_for_key(read_fd, 200);
        assert_eq!(bytes, b"x");
        // SAFETY: both fds are valid, open descriptors owned by this test.
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
        }
    }

    #[test]
    fn wait_for_key_times_out_on_an_empty_pipe() {
        let (read_fd, write_fd) = make_pipe();
        let start = std::time::Instant::now();
        let bytes = wait_for_key(read_fd, 80);
        assert!(bytes.is_empty());
        assert!(start.elapsed() >= std::time::Duration::from_millis(80));
        // SAFETY: both fds are valid, open descriptors owned by this test.
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
        }
    }
}
