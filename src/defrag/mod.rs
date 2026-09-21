//! `bios defrag`: the classic block grid, except every block is real disk usage. The state
//! machine is in `model`, the drawing in `view`, both pure. This module is the only part that
//! touches a filesystem or a terminal, and its one job beyond the loop is handing the terminal
//! back exactly as it was found, on every way out, including a panic.

pub mod model;
pub mod view;

use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use model::{Entry, Map};

/// How often the fill advances, in milliseconds: forty steps at this rate is about two seconds.
const FILL_TICK_MS: u64 = 50;
/// The fill reveals roughly a fortieth of the grid every tick, so the whole thing takes about two
/// seconds regardless of how big the grid is.
const FILL_STEPS: usize = 40;
/// At most this many of a directory's top level entries get their own colour, the six largest.
const MAX_ENTRIES: usize = 6;

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

/// `bios defrag [PATH]`. `path` defaults to the current directory. Returns the process exit code.
pub fn run(path: Option<&Path>) -> i32 {
    let target = match resolve_dir(path) {
        Ok(dir) => dir,
        Err(message) => {
            eprintln!("{message}");
            return 1;
        }
    };

    let mut entries = top_level_entries(&target);
    if is_nothing_to_defragment(&entries) {
        println!("{}", view::NOTHING_TO_DEFRAGMENT);
        return 0;
    }
    entries.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    entries.truncate(MAX_ENTRIES);

    let not_a_terminal = "bios: defrag needs a terminal. This is not one.";
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
        eprintln!("bios: defrag needs a terminal of at least 80x24. Yours is {cols}x{rows}.");
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
    let label = display_path(&target);
    // The seed only decides the shuffled fill order, not anything a test cannot pin; see
    // `setup::launch_memory_test`'s own seed for the same shape of impurity.
    let seed = crate::clock::now_unix();

    let Some(_raw) = crate::tty::RawGuard::new(fd) else {
        eprintln!("{not_a_terminal}");
        return 1;
    };
    let mut screen = Screen::enter(write_tty);
    run_loop(&mut screen, fd, cols, rows, &label, entries, seed, no_color);
    // The guards drop here, so the terminal is itself again before anything below could print.
    0
}

#[allow(clippy::too_many_arguments)]
fn run_loop<W: Write>(
    screen: &mut Screen<W>,
    fd: i32,
    cols: u16,
    rows: u16,
    label: &str,
    entries: Vec<Entry>,
    seed: u64,
    no_color: bool,
) {
    let (grid_cols, grid_rows) = view::grid_size(cols as usize, rows as usize);
    let mut map = Map::new(entries, grid_cols * grid_rows, seed);
    let step = (map.grid() / FILL_STEPS).max(1);

    loop {
        screen.draw(&view::render(
            &map,
            label,
            cols as usize,
            rows as usize,
            no_color,
        ));
        if map.done() {
            wait_for_any_key(fd);
            return;
        }
        let bytes = crate::tty::wait_for_key(fd, FILL_TICK_MS);
        if is_escape_or_ctrl_c(&bytes) {
            return;
        }
        map.step(step);
    }
}

/// Blocks, in the same bounded slices as the rest of the reader, until a key arrives.
fn wait_for_any_key(fd: i32) {
    while crate::tty::wait_for_key(fd, 250).is_empty() {}
}

/// Whether `bytes` opens with Esc or Ctrl-C: the two keys that leave early, mid fill, rather than
/// waiting for the "Any key" of a finished map. Any other key is ignored while filling; the plan
/// only promises "any key" once the map is done.
fn is_escape_or_ctrl_c(bytes: &[u8]) -> bool {
    matches!(bytes.first(), Some(0x1b) | Some(0x03))
}

/// Whether there is nothing worth drawing a map over: no top level entries at all, or entries
/// whose real disk usage sums to nothing measurable.
fn is_nothing_to_defragment(entries: &[Entry]) -> bool {
    entries.is_empty() || entries.iter().map(|e| e.bytes).sum::<u64>() == 0
}

/// `path`, or the current directory when there is none, resolved to a real, existing directory.
/// An error names what went wrong; `run` prints it and exits 1 rather than opening a screen over
/// nothing.
fn resolve_dir(path: Option<&Path>) -> Result<PathBuf, String> {
    let target = match path {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|_| "bios: cannot read the current directory.".to_string())?,
    };
    let canonical = std::fs::canonicalize(&target)
        .map_err(|_| format!("bios: {} is not a directory.", target.display()))?;
    if !canonical.is_dir() {
        return Err(format!("bios: {} is not a directory.", canonical.display()));
    }
    Ok(canonical)
}

/// Every direct child of `dir`, each with its own real disk usage: the sum of every block it (and
/// everything under it, for a directory) actually occupies. An unreadable directory (permissions,
/// or it vanished under us) is simply empty rather than an error: the caller turns that into the
/// same "nothing to defragment" message a genuinely empty directory gets.
fn top_level_entries(dir: &Path) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    read.flatten()
        .map(|child| Entry {
            name: child.file_name().to_string_lossy().into_owned(),
            bytes: disk_usage_bytes(&child.path()),
        })
        .collect()
}

/// Real disk usage of `path`, in bytes: every 512 byte block it is actually allocated, summed
/// recursively for a directory. This is `du`'s own measure, not a file's apparent length, which is
/// why an empty file or an otherwise mostly empty directory can still cost a block or more.
///
/// Symlinks are measured, never followed, both so a cycle of them can never recurse forever and
/// because that is what `du` itself does by default.
fn disk_usage_bytes(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    let mut total = meta.blocks() * 512;
    if meta.is_dir() {
        if let Ok(read) = std::fs::read_dir(path) {
            for child in read.flatten() {
                total += disk_usage_bytes(&child.path());
            }
        }
    }
    total
}

/// `path`, with a leading `$HOME` replaced by `~`, the same substitution the plan's own
/// `extras/ultra/ultra.zsh` reference makes on `$PWD`. Split out from `display_path` so a test can
/// hand it a `home` of its own rather than the real environment's.
fn display_path_with_home(path: &Path, home: Option<&str>) -> String {
    let raw = path.to_string_lossy().into_owned();
    match home {
        Some(home) if !home.is_empty() => match raw.strip_prefix(home) {
            Some(rest) => format!("~{rest}"),
            None => raw,
        },
        _ => raw,
    }
}

/// `path` as the header shows it: see `display_path_with_home`.
fn display_path(path: &Path) -> String {
    display_path_with_home(path, std::env::var("HOME").ok().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

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

    fn entry(name: &str, bytes: u64) -> Entry {
        Entry {
            name: name.to_string(),
            bytes,
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
    fn esc_ends_the_fill_early_and_restores_the_terminal() {
        let (read_fd, write_fd) = make_pipe();
        let buf = Arc::new(Mutex::new(Vec::new()));
        let mut screen = Screen::enter(SharedBuf(buf.clone()));
        // SAFETY: `write_fd` is a valid, open, writable fd from the pipe just created above.
        unsafe {
            libc::write(write_fd, [0x1bu8].as_ptr() as *const libc::c_void, 1);
        }
        run_loop(
            &mut screen,
            read_fd,
            80,
            24,
            "~/code/project",
            vec![entry("src", 1_000_000)],
            1,
            false,
        );
        drop(screen);
        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            out.ends_with("\x1b[0m\x1b[?25h\x1b[?1049l"),
            "esc should restore the terminal even mid fill: {out:?}"
        );
        close_pipe(read_fd, write_fd);
    }

    #[test]
    fn ctrl_c_also_ends_the_fill_early() {
        let (read_fd, write_fd) = make_pipe();
        let buf = Arc::new(Mutex::new(Vec::new()));
        let mut screen = Screen::enter(SharedBuf(buf.clone()));
        // SAFETY: `write_fd` is a valid, open, writable fd from the pipe just created above.
        unsafe {
            libc::write(write_fd, [0x03u8].as_ptr() as *const libc::c_void, 1);
        }
        run_loop(
            &mut screen,
            read_fd,
            80,
            24,
            "~/code/project",
            vec![entry("src", 1_000_000)],
            1,
            false,
        );
        drop(screen);
        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(out.ends_with("\x1b[0m\x1b[?25h\x1b[?1049l"));
        close_pipe(read_fd, write_fd);
    }

    #[test]
    fn is_nothing_to_defragment_is_true_for_empty_or_all_zero_entries() {
        assert!(is_nothing_to_defragment(&[]));
        assert!(is_nothing_to_defragment(&[entry("a", 0), entry("b", 0)]));
        assert!(!is_nothing_to_defragment(&[entry("a", 0), entry("b", 1)]));
    }

    #[test]
    fn display_path_replaces_a_leading_home_with_a_tilde() {
        assert_eq!(
            display_path_with_home(Path::new("/Users/rod/code/project"), Some("/Users/rod")),
            "~/code/project"
        );
    }

    #[test]
    fn display_path_leaves_a_path_outside_home_alone() {
        assert_eq!(
            display_path_with_home(Path::new("/var/tmp/project"), Some("/Users/rod")),
            "/var/tmp/project"
        );
    }

    #[test]
    fn display_path_with_no_home_is_the_raw_path() {
        assert_eq!(
            display_path_with_home(Path::new("/var/tmp/project"), None),
            "/var/tmp/project"
        );
    }

    #[test]
    fn resolve_dir_of_a_real_directory_canonicalises_it() {
        let dir = tempfile::tempdir().unwrap();
        let resolved = resolve_dir(Some(dir.path())).unwrap();
        assert_eq!(resolved, std::fs::canonicalize(dir.path()).unwrap());
    }

    #[test]
    fn resolve_dir_of_a_missing_path_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");
        assert!(resolve_dir(Some(&missing)).is_err());
    }

    #[test]
    fn resolve_dir_of_a_file_rather_than_a_directory_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not-a-directory");
        std::fs::write(&file, b"hi").unwrap();
        assert!(resolve_dir(Some(&file)).is_err());
    }

    #[test]
    fn an_empty_directory_has_no_top_level_entries() {
        let dir = tempfile::tempdir().unwrap();
        assert!(top_level_entries(dir.path()).is_empty());
    }

    #[test]
    fn a_larger_file_reports_more_disk_usage_than_a_tiny_one() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("tiny"), [0u8; 10]).unwrap();
        std::fs::write(dir.path().join("big"), vec![0u8; 2_000_000]).unwrap();

        let entries = top_level_entries(dir.path());
        let tiny = entries.iter().find(|e| e.name == "tiny").unwrap();
        let big = entries.iter().find(|e| e.name == "big").unwrap();
        assert!(
            big.bytes > tiny.bytes,
            "{big:?} should be bigger than {tiny:?}"
        );
    }

    #[test]
    fn a_directorys_usage_includes_what_is_nested_inside_it() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("payload"), vec![0u8; 3_000_000]).unwrap();

        let entries = top_level_entries(dir.path());
        let nested_entry = entries.iter().find(|e| e.name == "nested").unwrap();
        assert!(
            nested_entry.bytes >= 3_000_000,
            "the nested file's own size should be counted: {nested_entry:?}"
        );
    }
}
