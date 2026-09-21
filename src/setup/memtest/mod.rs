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

/// One byte sequence from the terminal, decoded: a key, the same as ever, or a mouse report. Kept
/// separate from `model::Key` because a mouse report carries a position a key press never does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Input {
    Key(Key),
    /// `col` is the pointer's column, in the terminal's own coordinates, one indexed, the same as
    /// the SGR report itself. Turning it into the field's own coordinates is `run`'s job, once it
    /// knows where the field sits on screen; `decode` below never sees that.
    Mouse {
        col: i32,
        left_click: bool,
    },
}

/// Turns on mouse reporting for the life of one game, and off again on every way out: a normal
/// end, giving up, Ctrl-C, and a panic unwinding through here, the same guarantee `Screen` and
/// `RawGuard` in `setup::mod` already give the rest of the terminal. There is exactly one place
/// mouse mode is ever turned on or off, this one, so there is nothing else that has to be kept in
/// sync with it.
///
/// Writes straight to `fd`, the same descriptor `setup::run` opened `/dev/tty` on and reads keys
/// from, rather than through `Screen`: `fd` and `Screen`'s own handle are two descriptors on the
/// same open file (`tty.try_clone()`, one kept for reading, one handed to `Screen` for writing),
/// so either reaches the terminal, and writing here means this guard need not borrow `Screen` for
/// the whole game just to turn one thing off again at the end.
struct MouseGuard {
    fd: i32,
}

impl MouseGuard {
    /// 1002 asks for the pointer's movement only while a button is held, not every movement
    /// anywhere over the terminal: press and drag steers the paddle, release and the keyboard is
    /// the only thing moving it again, the same as Breakout's own knob was something a player
    /// held onto to use, not something merely sitting near. The alternative, 1003 (every
    /// movement, button or not), would move the paddle out from under a keyboard player any time
    /// the pointer happened to drift over the field, with no way to tell why. 1006 asks for SGR
    /// extended coordinates, whose column and row arrive as plain decimal numbers rather than
    /// single bytes, so a field beyond 223 columns (impossible today, since the field is capped,
    /// but the encoding costs nothing extra) would still decode correctly rather than wrapping
    /// the way the older, plainer mode does.
    fn enable(fd: i32) -> MouseGuard {
        write_raw(fd, "\x1b[?1002h\x1b[?1006h");
        MouseGuard { fd }
    }
}

impl Drop for MouseGuard {
    fn drop(&mut self) {
        write_raw(self.fd, "\x1b[?1006l\x1b[?1002l");
    }
}

/// Writes `s` straight to `fd`. Failures are ignored, the same as `Screen::write`: there is
/// nothing useful to do about a write to a terminal failing, and the attempt must always be made.
fn write_raw(fd: i32, s: &str) {
    // SAFETY: `fd` is the caller's own file descriptor, open for writing for the life of the
    // game; `s.as_ptr()` and `s.len()` describe that same string's own bytes, valid for the call.
    unsafe {
        libc::write(fd, s.as_ptr() as *const libc::c_void, s.len());
    }
}

/// Whether the front of `pending` is a lone escape, or the start of an SGR mouse report that has
/// not seen its terminator yet: either way, more bytes of the same one input are still on their
/// way in, so the caller should wait a little longer for them rather than decoding what has
/// arrived so far as something else. Mouse reports run longer than a key's own escape sequences,
/// so they are more likely to land split across two reads.
fn looks_incomplete(pending: &[u8]) -> bool {
    pending == [0x1b]
        || (pending.starts_with(b"\x1b[<") && !pending.iter().any(|&b| b == b'M' || b == b'm'))
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
    let _mouse = MouseGuard::enable(fd);
    let (origin_col, _origin_row) = view::field_origin(&game, cols as usize, rows as usize);
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
        // A lone escape both is a key and begins every arrow key and every mouse report; give the
        // rest a moment to turn up before treating it as Esc on its own, same as setup's own loop.
        if looks_incomplete(&pending) {
            pending.extend_from_slice(&crate::tty::wait_for_key(fd, super::ESCAPE_GRACE_MS));
        }

        let mut inputs: Vec<Input> = Vec::new();
        while !pending.is_empty() {
            let Some((input, used)) = decode(&pending) else {
                pending.remove(0);
                continue;
            };
            pending.drain(..used);
            inputs.push(input);
        }

        // One redraw for everything that happened between the last tick and this one, never one
        // per input: a drag can turn up several mouse reports in a single read, and a redraw is a
        // full clear and rewrite of the screen, so that has to fold into the one draw the game's
        // own 30fps already budgets for, not multiply by however many reports arrived.
        let Some((needs_redraw, ended)) = advance(&mut game, &inputs, origin_col) else {
            // Ctrl-C, or the result screen being dismissed. Nothing to record either way.
            return;
        };

        if ended {
            save(&game);
            redraw(screen, &game);
            wait_for_any_key(fd);
            return;
        }

        if needs_redraw {
            redraw(screen, &game);
        }
    }
}

/// Applies every input decoded since the last tick, in order, then ticks the game once: the unit
/// of work between one read of the terminal and the next, and so also the unit a redraw is
/// coalesced to (see `run`, the only caller). `None` when any input asked to leave (Ctrl-C, or
/// dismissing the result screen), which `run` acts on at once rather than folding into this.
/// Otherwise, whether anything worth a redraw happened at all, and whether the game just ended.
fn advance(game: &mut Game, inputs: &[Input], origin_col: usize) -> Option<(bool, bool)> {
    let mut needs_redraw = false;
    let mut ended = false;
    for &input in inputs {
        match apply_input(game, input, origin_col) {
            Effect::Nothing => {}
            Effect::Redraw => needs_redraw = true,
            Effect::Ended => ended = true,
            Effect::Exit => return None,
        }
    }
    match game.tick() {
        Effect::Nothing => {}
        Effect::Redraw => needs_redraw = true,
        Effect::Ended => ended = true,
        // tick() never actually produces this, but the match has to be exhaustive.
        Effect::Exit => return None,
    }
    Some((needs_redraw, ended))
}

/// Applies one decoded `input` to `game`, turning a mouse report's absolute terminal column into
/// the field's own by subtracting `origin_col` (the field's own left edge, in those same terminal
/// coordinates) first.
fn apply_input(game: &mut Game, input: Input, origin_col: usize) -> Effect {
    match input {
        Input::Key(key) => game.key(key),
        Input::Mouse { col, left_click } => {
            let field_col = col - 1 - origin_col as i32;
            let move_effect = game.mouse_move(field_col);
            if !left_click {
                return move_effect;
            }
            // A click also serves the ball, the same as Space: whichever of the two actually did
            // something wins, so a click that both moves the paddle and launches still redraws,
            // and a click on the result screen still dismisses it.
            match game.key(Key::Space) {
                Effect::Nothing => move_effect,
                click_effect => click_effect,
            }
        }
    }
}

/// Blocks, in the same bounded slices as the rest of the reader, until a real key or a click
/// arrives. Mere mouse movement does not count: it is not a press of anything, and a result the
/// player is still reading should not vanish under a pointer that only happened to wander across
/// the terminal.
fn wait_for_any_key(fd: i32) {
    let mut pending: Vec<u8> = Vec::new();
    loop {
        pending.extend_from_slice(&crate::tty::wait_for_key(fd, 250));
        if looks_incomplete(&pending) {
            pending.extend_from_slice(&crate::tty::wait_for_key(fd, super::ESCAPE_GRACE_MS));
        }
        while !pending.is_empty() {
            let Some((input, used)) = decode(&pending) else {
                pending.remove(0);
                continue;
            };
            pending.drain(..used);
            match input {
                Input::Key(_) => return,
                Input::Mouse {
                    left_click: true, ..
                } => return,
                Input::Mouse {
                    left_click: false, ..
                } => {}
            }
        }
    }
}

/// Turns bytes from the terminal into one decoded input. `None` when the front of `bytes` is
/// neither a key nor a complete mouse report acted on, in which case the caller drops the first
/// byte and tries again.
fn decode(bytes: &[u8]) -> Option<(Input, usize)> {
    if bytes.starts_with(b"\x1b[<") {
        // Either a complete mouse report, or (`decode_mouse` returning `None`) still waiting on
        // its terminator: either way this is never treated as a lone Esc just because it starts
        // with one, the fallback below is only for bytes that are not the start of a mouse report
        // at all.
        return decode_mouse(bytes);
    }
    match bytes {
        [] => None,
        [0x1b, b'[', b'D', ..] => Some((Input::Key(Key::Left), 3)),
        [0x1b, b'[', b'C', ..] => Some((Input::Key(Key::Right), 3)),
        // An escape with nothing following it, or followed by something that is not the arrow
        // sequences or the mouse reports above, is Esc itself.
        [0x1b, ..] => Some((Input::Key(Key::Esc), 1)),
        [0x03, ..] => Some((Input::Key(Key::CtrlC), 1)),
        [b' ', ..] => Some((Input::Key(Key::Space), 1)),
        [b'a', ..] | [b'A', ..] => Some((Input::Key(Key::Left), 1)),
        [b'd', ..] | [b'D', ..] => Some((Input::Key(Key::Right), 1)),
        _ => None,
    }
}

/// Parses one SGR mouse report, `ESC [ < Cb ; Cx ; Cy` then `M` for a press or a movement, `m`
/// for a release, from the front of `bytes`. `None` when the front is not the start of one, or
/// its terminator has not arrived yet, in which case `decode`'s caller falls back to its usual
/// byte at a time recovery, the same as for anything else it does not recognise.
fn decode_mouse(bytes: &[u8]) -> Option<(Input, usize)> {
    let rest = bytes.strip_prefix(b"\x1b[<")?;
    let end = rest.iter().position(|&b| b == b'M' || b == b'm')?;
    let body = std::str::from_utf8(&rest[..end]).ok()?;
    let mut fields = body.split(';');
    let button: i32 = fields.next()?.parse().ok()?;
    let col: i32 = fields.next()?.parse().ok()?;
    let _row: i32 = fields.next()?.parse().ok()?;
    if fields.next().is_some() {
        return None;
    }
    // Button 0 (left), reported as a press, with neither the motion bit (32) nor any modifier
    // set: a plain left click. Everything else, motion, a drag, the other buttons, a release,
    // still repositions the paddle; only this one also serves the ball.
    let left_click = rest[end] == b'M' && button == 0;
    Some((Input::Mouse { col, left_click }, 3 + end + 1))
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
        assert_eq!(decode(b"\x1b[D"), Some((Input::Key(Key::Left), 3)));
        assert_eq!(decode(b"\x1b[C"), Some((Input::Key(Key::Right), 3)));
        assert_eq!(decode(b"a"), Some((Input::Key(Key::Left), 1)));
        assert_eq!(decode(b"A"), Some((Input::Key(Key::Left), 1)));
        assert_eq!(decode(b"d"), Some((Input::Key(Key::Right), 1)));
        assert_eq!(decode(b"D"), Some((Input::Key(Key::Right), 1)));
    }

    #[test]
    fn space_launches_and_the_two_ways_out_are_told_apart() {
        assert_eq!(decode(b" "), Some((Input::Key(Key::Space), 1)));
        assert_eq!(decode(b"\x1b"), Some((Input::Key(Key::Esc), 1)));
        // Ctrl-C is its own key, not another Esc: one scores what you cleared, one does not.
        assert_eq!(decode(b"\x03"), Some((Input::Key(Key::CtrlC), 1)));
    }

    #[test]
    fn a_key_the_game_does_not_act_on_is_simply_not_a_key() {
        assert_eq!(decode(b"z"), None);
        assert_eq!(decode(b""), None);
    }

    #[test]
    fn a_left_click_decodes_as_a_click_at_its_own_column() {
        let bytes = b"\x1b[<0;42;10M";
        assert_eq!(
            decode(bytes),
            Some((
                Input::Mouse {
                    col: 42,
                    left_click: true
                },
                bytes.len()
            ))
        );
    }

    #[test]
    fn a_release_or_a_drag_moves_the_paddle_but_never_clicks() {
        // A release of the left button.
        let release = b"\x1b[<0;42;10m";
        assert_eq!(
            decode(release),
            Some((
                Input::Mouse {
                    col: 42,
                    left_click: false
                },
                release.len()
            ))
        );
        // A drag: the left button still down, the motion bit (32) set. Mode 1002 (button motion
        // tracking, see `MouseGuard::enable`) only ever reports movement shaped like this one, a
        // button already held; it never reports plain motion with nothing pressed at all, which
        // is deliberate: a keyboard player's pointer merely resting over the field must never
        // steal the paddle out from under them.
        let drag = b"\x1b[<32;7;3M";
        assert_eq!(
            decode(drag),
            Some((
                Input::Mouse {
                    col: 7,
                    left_click: false
                },
                drag.len()
            ))
        );
        // The right button, not the left.
        let right_button = b"\x1b[<2;7;3M";
        assert_eq!(
            decode(right_button),
            Some((
                Input::Mouse {
                    col: 7,
                    left_click: false
                },
                right_button.len()
            ))
        );
    }

    #[test]
    fn an_incomplete_mouse_report_is_not_decoded_yet() {
        assert_eq!(decode(b"\x1b[<0;4"), None);
        assert!(looks_incomplete(b"\x1b[<0;4"));
        assert!(!looks_incomplete(b"\x1b[<0;4;9M"));
    }

    #[test]
    fn a_click_left_of_the_field_still_moves_the_paddle_clamped_into_it() {
        let mut g = Game::new(100, 0, false, 1, 80, 24);
        let effect = apply_input(
            &mut g,
            Input::Mouse {
                col: 1,
                left_click: false,
            },
            2,
        );
        assert_eq!(effect, Effect::Redraw);
        assert_eq!(g.paddle_col, 0);
    }

    #[test]
    fn a_left_click_serves_the_ball_even_when_the_paddle_does_not_need_to_move() {
        let mut g = Game::new(100, 0, false, 1, 80, 24);
        let origin_col = 2;
        let already_centred_col = g.paddle_col + model::PADDLE_WIDTH / 2 + 1 + origin_col;
        let effect = apply_input(
            &mut g,
            Input::Mouse {
                col: already_centred_col as i32,
                left_click: true,
            },
            origin_col,
        );
        assert_eq!(effect, Effect::Redraw, "the click itself still launches");
        assert_eq!(g.phase, model::Phase::Playing);
    }

    #[test]
    fn a_mouse_move_with_no_click_never_launches_the_ball() {
        let mut g = Game::new(100, 0, false, 1, 80, 24);
        apply_input(
            &mut g,
            Input::Mouse {
                col: 30,
                left_click: false,
            },
            2,
        );
        assert_eq!(g.phase, model::Phase::Ready);
    }

    #[test]
    fn a_sustained_drags_many_reports_between_two_ticks_still_yield_one_redraw() {
        let mut g = Game::new(100, 0, false, 1, 80, 24);
        let origin_col: usize = 2;
        // Fifty drag reports, the shape a real drag delivers many times a second, all landing in
        // one read before the next tick: `advance` is the only place a redraw is ever decided
        // from, and it returns one bool, not a count, so there is no way for fifty reports to
        // become fifty redraws.
        let inputs: Vec<Input> = (0..50i32)
            .map(|i| Input::Mouse {
                col: origin_col as i32 + 1 + i,
                left_click: false,
            })
            .collect();
        let (needs_redraw, ended) =
            advance(&mut g, &inputs, origin_col).expect("no input here asks to leave");
        assert!(needs_redraw, "the paddle did move, so one redraw is owed");
        assert!(!ended);
        // Every report still landed: the paddle ends up under the last one, the field column 49
        // (`col - 1 - origin_col`, see `apply_input`), not the first or somewhere lost between.
        assert_eq!(g.paddle_col, 49 - model::PADDLE_WIDTH / 2);
    }

    #[test]
    fn advance_with_nothing_decoded_still_ticks_the_game_once() {
        let mut g = Game::new(100, 0, false, 1, 80, 24);
        g.key(model::Key::Space);
        let before = (g.ball_x, g.ball_y);
        let (_, ended) = advance(&mut g, &[], 2).unwrap();
        assert!(!ended);
        assert_ne!(
            (g.ball_x, g.ball_y),
            before,
            "a tick with no input still moves the ball"
        );
    }

    #[test]
    fn advance_reports_none_the_moment_any_input_asks_to_leave() {
        let mut g = Game::new(100, 0, false, 1, 80, 24);
        let inputs = [
            Input::Mouse {
                col: 30,
                left_click: false,
            },
            Input::Key(model::Key::CtrlC),
            // Never reached: `advance` returns as soon as the input above asks to leave.
            Input::Mouse {
                col: 60,
                left_click: false,
            },
        ];
        let moved_before_ctrl_c = g.paddle_col;
        assert_eq!(advance(&mut g, &inputs, 2), None);
        assert_ne!(
            g.paddle_col, moved_before_ctrl_c,
            "the first move still applied"
        );
    }
}
