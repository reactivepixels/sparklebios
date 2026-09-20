//! The Memory Test easter egg: `bios setup`'s hidden Breakout. The state machine is in `model`,
//! the drawing in `view`, both pure. This module is the only impure part: it reuses the alternate
//! screen and key reader `setup` already opened, runs its own loop while the game is in play, and
//! writes the result to `state.json` exactly once, at the one moment the game truly ends.

pub mod model;
pub mod view;

use model::{Effect, Game, Key};

/// Roughly thirty frames a second.
const TICK_MS: u64 = 33;

/// What the Memory Test needs from the real machine, `state.json`, and the terminal `setup::run`
/// already opened. Bundled into one value so `run` takes a reasonable number of arguments.
pub(super) struct Session {
    pub fd: i32,
    pub cols: u16,
    pub rows: u16,
    pub mem_kb: u64,
    pub best_kb: u64,
    pub cleared_before: bool,
    /// Decides the launch angle, since the model itself never generates randomness.
    pub seed: u64,
}

/// Plays one game to its end (or to giving up), on the screen and key reader `setup::run` already
/// has open.
pub(super) fn run(screen: &mut super::Screen, session: Session) {
    let Session {
        fd,
        cols,
        rows,
        mem_kb,
        best_kb,
        cleared_before,
        seed,
    } = session;
    let mut game = Game::new(mem_kb, best_kb, cleared_before, seed, cols, rows);
    // The same decision `bios boot`, `bios fetch` and the setup screen already make; see
    // `render::color_mode_from_env`.
    let no_color = crate::render::color_mode_from_env(
        std::env::var("NO_COLOR").ok().as_deref(),
        std::env::var("COLORTERM").ok().as_deref(),
    ) == crate::render::ColorMode::None;
    let redraw = |screen: &mut super::Screen, game: &Game| {
        screen.draw(&view::render(game, cols as usize, rows as usize, no_color));
    };
    redraw(screen, &game);

    let mut pending: Vec<u8> = Vec::new();
    loop {
        pending.extend_from_slice(&crate::tty::wait_for_key(fd, TICK_MS));
        // A lone escape both is a key and begins the arrow keys; give the rest a moment to turn
        // up before treating it as Esc on its own, same as setup's own loop.
        if pending == [0x1b] {
            pending.extend_from_slice(&crate::tty::wait_for_key(fd, super::ESCAPE_GRACE_MS));
        }

        let mut ended = false;
        while !pending.is_empty() {
            let Some((key, used)) = decode(&pending) else {
                pending.remove(0);
                continue;
            };
            pending.drain(..used);
            match game.key(key) {
                Effect::Nothing => {}
                Effect::Redraw => redraw(screen, &game),
                Effect::Ended => ended = true,
                // Ctrl-C, or the result screen being dismissed. Nothing to record either way.
                Effect::Exit => return,
            }
        }

        match game.tick() {
            Effect::Nothing => {}
            Effect::Redraw => redraw(screen, &game),
            Effect::Ended => ended = true,
            // tick() never actually produces this, but the match has to be exhaustive.
            Effect::Exit => return,
        }

        if ended {
            save(&game);
            redraw(screen, &game);
            wait_for_any_key(fd);
            return;
        }
    }
}

/// Blocks, in the same bounded slices as the rest of the reader, until a key arrives.
fn wait_for_any_key(fd: i32) {
    while crate::tty::wait_for_key(fd, 250).is_empty() {}
}

/// Turns bytes from the terminal into a game key. `None` when the bytes are not one we act on, in
/// which case the caller drops the first byte and tries again.
fn decode(bytes: &[u8]) -> Option<(Key, usize)> {
    match bytes {
        [] => None,
        [0x1b, b'[', b'D', ..] => Some((Key::Left, 3)),
        [0x1b, b'[', b'C', ..] => Some((Key::Right, 3)),
        // An escape with nothing following it, or followed by something that is not the arrow
        // sequences above, is Esc itself.
        [0x1b, ..] => Some((Key::Esc, 1)),
        [0x03, ..] => Some((Key::CtrlC, 1)),
        [b' ', ..] => Some((Key::Space, 1)),
        [b'a', ..] | [b'A', ..] => Some((Key::Left, 1)),
        [b'd', ..] | [b'D', ..] => Some((Key::Right, 1)),
        _ => None,
    }
}

/// Writes the result to `state.json`, exactly once. Missing only when there is nowhere to write
/// it, in which case there is nothing useful to do about it.
fn save(game: &Game) {
    let Some(dir) = crate::paths::state_dir() else {
        return;
    };
    let mut state = crate::state::State::load(&dir);
    let score = game.score();
    if score > state.memory_test_best_kb {
        state.memory_test_best_kb = score;
    }
    if game.remaining_kb == 0 {
        state.memory_test_cleared = true;
    }
    let _ = state.save(&dir);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrow_keys_and_the_letters_do_the_same_thing() {
        assert_eq!(decode(b"\x1b[D"), Some((Key::Left, 3)));
        assert_eq!(decode(b"\x1b[C"), Some((Key::Right, 3)));
        assert_eq!(decode(b"a"), Some((Key::Left, 1)));
        assert_eq!(decode(b"A"), Some((Key::Left, 1)));
        assert_eq!(decode(b"d"), Some((Key::Right, 1)));
        assert_eq!(decode(b"D"), Some((Key::Right, 1)));
    }

    #[test]
    fn space_launches_and_the_two_ways_out_are_told_apart() {
        assert_eq!(decode(b" "), Some((Key::Space, 1)));
        assert_eq!(decode(b"\x1b"), Some((Key::Esc, 1)));
        // Ctrl-C is its own key, not another Esc: one scores what you cleared, one does not.
        assert_eq!(decode(b"\x03"), Some((Key::CtrlC, 1)));
    }

    #[test]
    fn a_key_the_game_does_not_act_on_is_simply_not_a_key() {
        assert_eq!(decode(b"z"), None);
        assert_eq!(decode(b""), None);
    }
}
