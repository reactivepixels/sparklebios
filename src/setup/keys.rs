//! Turning bytes from the terminal into keys. Pure, so the whole table is tested without a tty.

use super::model::Key;

/// What a press means to the setup screen. `Answer` only matters while a dialog is open, but it
/// is decoded the same way wherever it arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Key(Key),
    Answer(bool),
}

/// Decodes the first key in `bytes`, returning it and how many bytes it used. `None` when the
/// bytes are not a key we act on, in which case the caller drops the first byte and tries again.
///
/// A lone escape is the awkward one: it both is a key and starts every arrow key. The caller
/// resolves that by waiting briefly for more bytes before asking, so by the time a bare `0x1b`
/// reaches here with nothing after it, it really was Esc.
pub fn decode(bytes: &[u8]) -> Option<(Input, usize)> {
    let key = |k: Key, n: usize| Some((Input::Key(k), n));
    match bytes {
        [] => None,
        [0x1b, b'[', rest @ ..] => decode_csi(rest).map(|(input, n)| (input, n + 2)),
        // An escape with nothing following it is Esc itself.
        [0x1b] => key(Key::Esc, 1),
        // An escape followed by something that is not a sequence we know: treat the escape as
        // Esc and let the rest be read as its own keys.
        [0x1b, ..] => key(Key::Esc, 1),
        [0x03, ..] => key(Key::CtrlC, 1),
        [b'\r', ..] | [b'\n', ..] => key(Key::Enter, 1),
        [b'k', ..] => key(Key::Up, 1),
        [b'j', ..] => key(Key::Down, 1),
        [b'h', ..] | [b'-', ..] => key(Key::Left, 1),
        [b'l', ..] | [b'+', ..] => key(Key::Right, 1),
        [b'q', ..] | [b'Q', ..] => key(Key::Esc, 1),
        [b'y', ..] | [b'Y', ..] => Some((Input::Answer(true), 1)),
        [b'n', ..] | [b'N', ..] => Some((Input::Answer(false), 1)),
        _ => None,
    }
}

/// The part of a control sequence after `ESC [`.
fn decode_csi(rest: &[u8]) -> Option<(Input, usize)> {
    let key = |k: Key, n: usize| Some((Input::Key(k), n));
    match rest {
        [b'A', ..] => key(Key::Up, 1),
        [b'B', ..] => key(Key::Down, 1),
        [b'C', ..] => key(Key::Right, 1),
        [b'D', ..] => key(Key::Left, 1),
        // Page up and page down step a value, which is what they do in a real setup screen.
        [b'5', b'~', ..] => key(Key::Left, 2),
        [b'6', b'~', ..] => key(Key::Right, 2),
        [b'2', b'1', b'~', ..] => key(Key::F10, 3),
        // Delete. The boot screen invites you to press it, so it should not be a stray character.
        [b'3', b'~', ..] => key(Key::Esc, 2),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(bytes: &[u8]) -> Option<Input> {
        decode(bytes).map(|(input, _)| input)
    }

    #[test]
    fn the_arrow_keys() {
        assert_eq!(k(b"\x1b[A"), Some(Input::Key(Key::Up)));
        assert_eq!(k(b"\x1b[B"), Some(Input::Key(Key::Down)));
        assert_eq!(k(b"\x1b[C"), Some(Input::Key(Key::Right)));
        assert_eq!(k(b"\x1b[D"), Some(Input::Key(Key::Left)));
        assert_eq!(decode(b"\x1b[A").unwrap().1, 3);
    }

    #[test]
    fn the_vi_keys_do_the_same_thing() {
        assert_eq!(k(b"k"), Some(Input::Key(Key::Up)));
        assert_eq!(k(b"j"), Some(Input::Key(Key::Down)));
        assert_eq!(k(b"h"), Some(Input::Key(Key::Left)));
        assert_eq!(k(b"l"), Some(Input::Key(Key::Right)));
    }

    #[test]
    fn plus_and_minus_change_a_value() {
        assert_eq!(k(b"-"), Some(Input::Key(Key::Left)));
        assert_eq!(k(b"+"), Some(Input::Key(Key::Right)));
    }

    #[test]
    fn page_up_and_down_change_a_value_too() {
        assert_eq!(k(b"\x1b[5~"), Some(Input::Key(Key::Left)));
        assert_eq!(k(b"\x1b[6~"), Some(Input::Key(Key::Right)));
        assert_eq!(decode(b"\x1b[5~").unwrap().1, 4);
    }

    #[test]
    fn f10_saves() {
        assert_eq!(k(b"\x1b[21~"), Some(Input::Key(Key::F10)));
        assert_eq!(decode(b"\x1b[21~").unwrap().1, 5);
    }

    #[test]
    fn delete_leaves_rather_than_typing_a_stray_character() {
        assert_eq!(k(b"\x1b[3~"), Some(Input::Key(Key::Esc)));
    }

    #[test]
    fn enter_in_both_spellings() {
        assert_eq!(k(b"\r"), Some(Input::Key(Key::Enter)));
        assert_eq!(k(b"\n"), Some(Input::Key(Key::Enter)));
    }

    #[test]
    fn a_lone_escape_is_escape() {
        assert_eq!(k(b"\x1b"), Some(Input::Key(Key::Esc)));
        assert_eq!(decode(b"\x1b").unwrap().1, 1);
    }

    #[test]
    fn q_leaves_and_ctrl_c_leaves_harder() {
        assert_eq!(k(b"q"), Some(Input::Key(Key::Esc)));
        assert_eq!(k(b"Q"), Some(Input::Key(Key::Esc)));
        assert_eq!(k(b"\x03"), Some(Input::Key(Key::CtrlC)));
    }

    #[test]
    fn y_and_n_answer_a_dialog_in_either_case() {
        assert_eq!(k(b"y"), Some(Input::Answer(true)));
        assert_eq!(k(b"Y"), Some(Input::Answer(true)));
        assert_eq!(k(b"n"), Some(Input::Answer(false)));
        assert_eq!(k(b"N"), Some(Input::Answer(false)));
    }

    #[test]
    fn a_key_we_do_not_act_on_is_simply_not_a_key() {
        assert_eq!(k(b"z"), None);
        assert_eq!(k(b""), None);
        assert_eq!(k(b"\x1b[Z"), None, "shift-tab is not one of ours");
    }

    #[test]
    fn an_unknown_escape_sequence_does_not_swallow_what_follows() {
        // ESC [ Z is not ours, so the escape is taken as Esc and the rest read separately,
        // rather than the whole thing vanishing.
        assert_eq!(k(b"\x1bX"), Some(Input::Key(Key::Esc)));
        assert_eq!(decode(b"\x1bX").unwrap().1, 1);
    }

    #[test]
    fn a_sequence_is_decoded_from_the_front_of_a_longer_buffer() {
        let (input, used) = decode(b"\x1b[Bjjj").unwrap();
        assert_eq!(input, Input::Key(Key::Down));
        assert_eq!(used, 3, "only the sequence itself is consumed");
    }
}
