//! `bios screensaver`: the wordmark bounces around the screen, DVD style, sparkling as it goes,
//! until any key is pressed. The state machine is in `model`, the drawing in `view`, both pure.
//! This module is the only part that touches a terminal, and its one job beyond the loop is
//! handing the terminal back exactly as it was found, on every way out, including a panic.

pub mod model;
pub mod view;

use std::io::Write;
use std::os::unix::io::AsRawFd;

use model::{Image, Kind, Model};

/// About fifteen frames a second.
const FRAME_MS: u64 = 66;

/// The first Kitty image id this screen's fourteen images occupy, one per image, counting up
/// from `Image::offset`. Outside the 9000 to 9899 range the shell prompt's own mascot ids live
/// in (see `cli::image_for`), and short enough of 9999 to leave room for all fourteen, so the
/// whole block still sits clear of that range.
const IMAGE_ID_BASE: u32 = 9986;

/// The Kitty image id `image` is transmitted under and, later, placed by.
fn image_id(image: Image) -> u32 {
    IMAGE_ID_BASE + image.offset()
}

/// `image`'s own PNG bytes, real pixels, transmitted once per run and then only ever placed
/// again by id. See `sprites/wordmark/`.
fn image_bytes(image: Image) -> &'static [u8] {
    match image {
        Image::Rainbow => include_bytes!("../../sprites/wordmark/rainbow.png"),
        Image::Ripple1 => include_bytes!("../../sprites/wordmark/ripple1.png"),
        Image::Ripple2 => include_bytes!("../../sprites/wordmark/ripple2.png"),
        Image::Ripple3 => include_bytes!("../../sprites/wordmark/ripple3.png"),
        Image::Ripple4 => include_bytes!("../../sprites/wordmark/ripple4.png"),
        Image::Ripple5 => include_bytes!("../../sprites/wordmark/ripple5.png"),
        Image::Ripple6 => include_bytes!("../../sprites/wordmark/ripple6.png"),
        Image::Red => include_bytes!("../../sprites/wordmark/red.png"),
        Image::Yellow => include_bytes!("../../sprites/wordmark/yellow.png"),
        Image::Green => include_bytes!("../../sprites/wordmark/green.png"),
        Image::Cyan => include_bytes!("../../sprites/wordmark/cyan.png"),
        Image::Blue => include_bytes!("../../sprites/wordmark/blue.png"),
        Image::Magenta => include_bytes!("../../sprites/wordmark/magenta.png"),
        Image::White => include_bytes!("../../sprites/wordmark/white.png"),
    }
}

/// Seeds a sparkle decision from the clock, the same way `boot::seed_from_time` seeds the quip
/// rotation: read once, at the start of a run, then advanced by hand from there (see
/// `model::Model::next_random`). A test hands `Model::new` a fixed number instead.
fn seed_from_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Puts the terminal on the alternate screen with the cursor hidden, and puts it back on drop, on
/// every way out including a panic unwinding through it. Generic over the writer so a test can
/// hand it an in-memory buffer instead of a real terminal; `run` always instantiates it over a
/// `std::fs::File`.
struct Screen<W: Write> {
    out: W,
}

impl<W: Write> Screen<W> {
    fn enter(out: W) -> Screen<W> {
        let mut screen = Screen { out };
        screen.write("\x1b[?1049h\x1b[?25l\x1b[2J");
        screen
    }

    fn write(&mut self, s: &str) {
        let _ = self.out.write_all(s.as_bytes());
        let _ = self.out.flush();
    }

    /// Redraws from the top left. The screen is always drawn whole, so there is nothing to erase
    /// beyond the clear.
    fn draw(&mut self, body: &str) {
        self.write("\x1b[H\x1b[2J");
        self.write(body);
    }
}

impl<W: Write> Drop for Screen<W> {
    fn drop(&mut self) {
        // Cursor shown, off the alternate screen, attributes reset. Failures are ignored: there
        // is nothing useful to do about them, and the attempt must always be made.
        self.write("\x1b[0m\x1b[?25h\x1b[?1049l");
    }
}

/// `bios screensaver`. Returns the process exit code.
pub fn run() -> i32 {
    let not_a_terminal = "bios: screensaver needs a terminal. This is not one.";
    let Ok(tty) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
    else {
        eprintln!("{not_a_terminal}");
        return 1;
    };
    let fd = tty.as_raw_fd();
    // SAFETY: `fd` is a valid open file descriptor; `isatty` only reads it.
    if unsafe { libc::isatty(fd) } == 0 {
        eprintln!("{not_a_terminal}");
        return 1;
    }
    let Some((cols, rows)) = crate::term::size(fd) else {
        eprintln!("{not_a_terminal}");
        return 1;
    };
    if (cols as usize) < view::MIN_WIDTH || (rows as usize) < view::MIN_HEIGHT {
        eprintln!("bios: screensaver needs a terminal of at least 80x24. Yours is {cols}x{rows}.");
        return 1;
    }
    let Ok(write_tty) = tty.try_clone() else {
        eprintln!("{not_a_terminal}");
        return 1;
    };

    // The same decision `bios setup` and `bios boot` already make; see
    // `render::color_mode_from_env`.
    let no_color = crate::render::color_mode_from_env(
        std::env::var("NO_COLOR").ok().as_deref(),
        std::env::var("COLORTERM").ok().as_deref(),
    ) == crate::render::ColorMode::None;
    let protocol = crate::sprite::detect_image_protocol(
        std::env::var("TERM").ok().as_deref(),
        std::env::var("TERM_PROGRAM").ok().as_deref(),
        std::env::var("LC_TERMINAL").ok().as_deref(),
    );

    let Some(_raw) = crate::tty::RawGuard::new(fd) else {
        eprintln!("{not_a_terminal}");
        return 1;
    };
    let mut screen = Screen::enter(write_tty);
    run_loop(&mut screen, fd, cols, rows, no_color, protocol);
    // The guards drop here, so the terminal is itself again before anything below could print.
    0
}

fn run_loop<W: Write>(
    screen: &mut Screen<W>,
    fd: i32,
    cols: u16,
    rows: u16,
    no_color: bool,
    protocol: Option<crate::sprite::ImageProtocol>,
) {
    match protocol {
        Some(crate::sprite::ImageProtocol::Kitty) => run_image(screen, fd, cols, rows),
        // iTerm2's own protocol has no stored image to move: the picture itself is what gets
        // sent, every time (see `cli::image_for`'s own note on the same split). Redrawing that
        // fifteen times a second is exactly the "few hundred kilobytes of corrupt escapes a
        // second" mistake this screen must never repeat, so terminals that speak it fall back to
        // the plain text wordmark instead of the real one.
        _ => run_text(screen, fd, cols, rows, no_color),
    }
}

/// The text mode loop: draws a fresh frame (wordmark, ripple and twinkle all folded into the one
/// redraw by `view::render`), waits one frame for a key, steps the model, repeats.
fn run_text<W: Write>(screen: &mut Screen<W>, fd: i32, cols: u16, rows: u16, no_color: bool) {
    let mut model = Model::new(
        cols as i32,
        rows as i32,
        21,
        1,
        Kind::Text,
        seed_from_time(),
    );
    loop {
        screen.draw(&view::render(
            &model,
            cols as usize,
            rows as usize,
            no_color,
        ));
        if !crate::tty::wait_for_key(fd, FRAME_MS).is_empty() {
            return;
        }
        model.step();
    }
}

/// The image mode loop: transmits all fourteen images once, then every frame moves the
/// placement to whichever of them is current, rather than sending a picture again. Unlike text
/// mode this never redraws the whole screen, so a twinkle is drawn and, once it ends, cleared as
/// its own small escape; `last_twinkle` is what tells this loop the twinkle just ended, since by
/// then the model has already forgotten it.
fn run_image<W: Write>(screen: &mut Screen<W>, fd: i32, cols: u16, rows: u16) {
    let (width, height) = view::image_size(cols);
    let mut model = Model::new(
        cols as i32,
        rows as i32,
        width as i32,
        height as i32,
        Kind::Image,
        seed_from_time(),
    );
    for image in Image::ALL {
        screen.write(&crate::sprite::kitty_transmit(
            image_bytes(image),
            image_id(image),
        ));
    }
    let mut last_twinkle = None;
    loop {
        screen.write(&view::image_move(
            image_id(model.current_image()),
            width,
            height,
            model.x as u16,
            model.y as u16,
        ));
        match model.twinkle {
            Some(twinkle) => screen.write(&view::twinkle_frame_image(&twinkle)),
            None => {
                if let Some(ended) = last_twinkle {
                    screen.write(&view::twinkle_clear_image(ended));
                }
            }
        }
        last_twinkle = model.twinkle.map(|t| t.cells);
        if !crate::tty::wait_for_key(fd, FRAME_MS).is_empty() {
            return;
        }
        model.step();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// An in-memory writer several tests below share, standing in for the real terminal `run`
    /// would otherwise write to.
    #[derive(Clone)]
    struct SharedBuf(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedBuf {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn make_pipe() -> (i32, i32) {
        let mut fds = [0i32; 2];
        // SAFETY: `fds` is a valid, writable array of two `c_int`s.
        let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
        assert_eq!(rc, 0, "libc::pipe failed");
        (fds[0], fds[1])
    }

    fn close_pipe(read_fd: i32, write_fd: i32) {
        // SAFETY: both fds are valid, open descriptors owned by the calling test.
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
        }
    }

    #[test]
    fn the_screen_guard_restores_the_terminal_even_when_a_panic_unwinds_through_it() {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let shared = SharedBuf(buf.clone());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _screen = Screen::enter(shared);
            panic!("boom");
        }));
        assert!(result.is_err());
        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            out.ends_with("\x1b[0m\x1b[?25h\x1b[?1049l"),
            "the terminal was not restored: {out:?}"
        );
    }

    #[test]
    fn any_key_ends_the_text_loop_and_restores_the_terminal() {
        let (read_fd, write_fd) = make_pipe();
        let buf = Arc::new(Mutex::new(Vec::new()));
        let mut screen = Screen::enter(SharedBuf(buf.clone()));
        // A key already waiting means the loop's very first check ends it immediately.
        // SAFETY: `write_fd` is a valid, open, writable fd from the pipe just created above.
        unsafe {
            libc::write(write_fd, b"x".as_ptr() as *const libc::c_void, 1);
        }
        run_text(&mut screen, read_fd, 80, 24, false);
        drop(screen);
        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            out.ends_with("\x1b[0m\x1b[?25h\x1b[?1049l"),
            "esc, ctrl-c or any other key should leave the terminal exactly as it was found: {out:?}"
        );
        close_pipe(read_fd, write_fd);
    }

    #[test]
    fn the_image_loop_transmits_all_fourteen_images_exactly_once() {
        let (read_fd, write_fd) = make_pipe();
        let buf = Arc::new(Mutex::new(Vec::new()));
        let mut screen = Screen::enter(SharedBuf(buf.clone()));
        // SAFETY: `write_fd` is a valid, open, writable fd from the pipe just created above.
        unsafe {
            libc::write(write_fd, b"x".as_ptr() as *const libc::c_void, 1);
        }
        run_image(&mut screen, read_fd, 80, 24);
        drop(screen);
        let out = String::from_utf8_lossy(&buf.lock().unwrap()).to_string();
        // `a=t` transmits and stores an image; `a=p` places one. Only one frame is drawn here
        // (the key was already waiting), so fourteen transmits (one per image) and a single
        // placement is expected; the point of the test is that the transmits are not part of the
        // per frame path at all (see `run_image`: the `kitty_transmit` calls sit before the
        // loop, the `image_move` call inside it).
        assert_eq!(
            out.matches("a=t").count(),
            14,
            "transmitted a different number of images than fourteen: {out:?}"
        );
        assert_eq!(out.matches("a=p").count(), 1);
        close_pipe(read_fd, write_fd);
    }

    #[test]
    fn the_fourteen_transmissions_happen_once_for_a_whole_run_however_many_frames() {
        let (read_fd, write_fd) = make_pipe();
        let buf = Arc::new(Mutex::new(Vec::new()));
        let mut screen = Screen::enter(SharedBuf(buf.clone()));
        // No key waiting yet: `run_image` draws several frames, each a `FRAME_MS` wait, before
        // this arrives and ends it.
        let key_after_a_few_frames = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(FRAME_MS * 4));
            // SAFETY: `write_fd` is a valid, open, writable fd from the pipe created above, and
            // this thread is joined, below, before the test closes it.
            unsafe {
                libc::write(write_fd, b"x".as_ptr() as *const libc::c_void, 1);
            }
        });
        run_image(&mut screen, read_fd, 80, 24);
        key_after_a_few_frames.join().unwrap();
        drop(screen);
        let out = String::from_utf8_lossy(&buf.lock().unwrap()).to_string();
        assert_eq!(
            out.matches("a=t").count(),
            14,
            "the fourteen images should still transmit exactly once each: {out:?}"
        );
        assert!(
            out.matches("a=p").count() > 1,
            "should have drawn more than one frame while waiting for the key: {out:?}"
        );
        close_pipe(read_fd, write_fd);
    }
}
