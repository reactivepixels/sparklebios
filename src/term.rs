//! Terminal geometry.

/// Columns of the terminal on this fd via ioctl(TIOCGWINSZ). None when it is not a terminal.
pub fn cols(fd: i32) -> Option<u16> {
    // SAFETY: `ws` is a plain C struct of integer fields, so an all zero bit pattern is valid;
    // it is only read after `ioctl` has had a chance to fill it in.
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    // SAFETY: `fd` is the caller's file descriptor and `ws` points at a valid, writable
    // `winsize` we own for the duration of the call.
    let rc = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) };
    if rc != 0 || ws.ws_col == 0 {
        None
    } else {
        Some(ws.ws_col)
    }
}

/// Columns and rows of the terminal on this fd. None when it is not a terminal, or when it
/// reports a size of zero, which some terminals do before they have been sized.
pub fn size(fd: i32) -> Option<(u16, u16)> {
    // SAFETY: `ws` is a plain C struct of integer fields, so an all zero bit pattern is valid;
    // it is only read after `ioctl` has had a chance to fill it in.
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    // SAFETY: `fd` is the caller's file descriptor and `ws` points at a valid, writable
    // `winsize` we own for the duration of the call.
    let rc = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) };
    if rc != 0 || ws.ws_col == 0 || ws.ws_row == 0 {
        None
    } else {
        Some((ws.ws_col, ws.ws_row))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::io::AsRawFd;

    #[test]
    fn a_non_terminal_fd_yields_none() {
        let file = tempfile::tempfile().unwrap();
        assert_eq!(cols(file.as_raw_fd()), None);
        assert_eq!(size(file.as_raw_fd()), None);
    }
}
