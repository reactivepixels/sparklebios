//! The setup screen's state machine. Pure: no terminal, no files, no clock. Everything the
//! screen does in response to a key is decided here, so it can all be tested without a tty.

/// A key the setup screen understands. Anything else is ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Move the selection up.
    Up,
    /// Move the selection down.
    Down,
    /// Cycle the current row's value backward.
    Left,
    /// Cycle the current row's value forward.
    Right,
    /// Confirm the highlighted dialog choice.
    Enter,
    /// Save and leave.
    F10,
    /// Cancel a dialog, or open the quit dialog from the main screen.
    Esc,
    /// Leave at once, saving nothing.
    CtrlC,
}

/// What the caller should do after a key. `Redraw` is the ordinary answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Nothing changed; the screen does not need redrawing.
    Nothing,
    /// The screen changed and should be redrawn.
    Redraw,
    /// Leave the alternate screen, draw the boot screen as it would look, wait for a key.
    Preview,
    /// Save the changed settings and leave.
    Save,
    /// Leave without saving.
    Exit,
}

/// Which dialog, if any, is over the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialog {
    /// No dialog is open.
    None,
    /// Confirming whether to save before leaving.
    Save,
    /// Confirming whether to leave without saving.
    Quit,
}

/// The setting a row writes, or `None` for a row that is not saved at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    /// Which flavour boots.
    Flavour,
    /// Which Ghostty theme is active.
    Theme,
    /// The mascot graphics preference.
    Mascot,
    /// The sprinkles dial.
    Sprinkles,
    /// Whether the once-a-day animated show is allowed to play.
    DailyShow,
    /// The boot screen's master switch.
    BootScreen,
    /// Turbo. Engages nothing, and has done since 1995, but the flag is saved anyway, into
    /// `state.json` rather than the config file: it is the same flag `bios turbo` toggles.
    Turbo,
}

/// One line of the left pane: a label, the values it cycles through, and where it started.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The label shown in the left pane.
    pub label: &'static str,
    /// The setting this row writes, if any.
    pub setting: Setting,
    /// The values this row cycles through, in order.
    pub values: Vec<String>,
    /// The index into `values` currently shown.
    pub selected: usize,
    /// The index the row started at, so `changed` can tell if it moved.
    pub initial: usize,
    /// The help text shown in the right pane while this row is highlighted.
    pub help: &'static str,
}

impl Row {
    /// The currently selected value.
    pub fn value(&self) -> &str {
        &self.values[self.selected]
    }

    /// Whether the user has moved this row away from its starting value.
    pub fn changed(&self) -> bool {
        self.selected != self.initial
    }

    /// Moves to the next or previous value, wrapping at both ends.
    fn step(&mut self, forward: bool) {
        let n = self.values.len();
        if n <= 1 {
            return;
        }
        self.selected = if forward {
            (self.selected + 1) % n
        } else {
            (self.selected + n - 1) % n
        };
    }
}

/// The setup screen's whole state: its rows and whatever dialog is open.
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    /// Every row on the left pane, in display order.
    pub rows: Vec<Row>,
    /// The index into `rows` currently highlighted.
    pub selected: usize,
    /// Which dialog, if any, is over the screen.
    pub dialog: Dialog,
}

impl State {
    /// A fresh screen over `rows`, nothing highlighted but the first row, no dialog open.
    pub fn new(rows: Vec<Row>) -> Self {
        State {
            rows,
            selected: 0,
            dialog: Dialog::None,
        }
    }

    /// Every row whose value the user changed. Turbo is included: the flag it carries is saved
    /// like any other row's, just into a different file (see `Setting::Turbo`).
    pub fn changes(&self) -> Vec<(Setting, &str)> {
        self.rows
            .iter()
            .filter(|r| r.changed())
            .map(|r| (r.setting, r.value()))
            .collect()
    }

    /// Whether anything that would be written has been changed.
    pub fn dirty(&self) -> bool {
        !self.changes().is_empty()
    }

    /// The currently highlighted row.
    pub fn current(&self) -> &Row {
        &self.rows[self.selected]
    }

    /// Applies one key press, returning what the caller should do next.
    pub fn key(&mut self, key: Key) -> Effect {
        // Ctrl-C leaves at once from anywhere, saving nothing. It is the one key that does not
        // stop to ask, because somebody pressing it wants out.
        if key == Key::CtrlC {
            return Effect::Exit;
        }
        match self.dialog {
            Dialog::None => self.key_on_screen(key),
            Dialog::Save => self.key_in_save_dialog(key),
            Dialog::Quit => self.key_in_quit_dialog(key),
        }
    }

    fn key_on_screen(&mut self, key: Key) -> Effect {
        let n = self.rows.len();
        match key {
            Key::Up => {
                self.selected = (self.selected + n - 1) % n;
                Effect::Redraw
            }
            Key::Down => {
                self.selected = (self.selected + 1) % n;
                Effect::Redraw
            }
            Key::Left => {
                self.rows[self.selected].step(false);
                Effect::Redraw
            }
            Key::Right => {
                self.rows[self.selected].step(true);
                Effect::Redraw
            }
            Key::Enter => Effect::Preview,
            Key::F10 => {
                self.dialog = Dialog::Save;
                Effect::Redraw
            }
            // Nothing to lose means nothing to ask about.
            Key::Esc if !self.dirty() => Effect::Exit,
            Key::Esc => {
                self.dialog = Dialog::Quit;
                Effect::Redraw
            }
            Key::CtrlC => Effect::Exit,
        }
    }

    fn key_in_save_dialog(&mut self, key: Key) -> Effect {
        match key {
            // The dialog's default is Y, so Enter saves.
            Key::Enter => Effect::Save,
            Key::Esc => {
                self.dialog = Dialog::None;
                Effect::Redraw
            }
            _ => Effect::Nothing,
        }
    }

    fn key_in_quit_dialog(&mut self, key: Key) -> Effect {
        match key {
            // The dialog's default is N, so Enter goes back rather than losing the changes.
            Key::Enter | Key::Esc => {
                self.dialog = Dialog::None;
                Effect::Redraw
            }
            _ => Effect::Nothing,
        }
    }

    /// Y or N in whichever dialog is open. Kept apart from `key` because the letters mean
    /// nothing on the screen itself, and a letter that means nothing should not be invented into
    /// a key the state machine has to know about.
    pub fn answer(&mut self, yes: bool) -> Effect {
        match self.dialog {
            Dialog::Save if yes => Effect::Save,
            Dialog::Save => {
                self.dialog = Dialog::None;
                Effect::Redraw
            }
            Dialog::Quit if yes => Effect::Exit,
            Dialog::Quit => {
                self.dialog = Dialog::None;
                Effect::Redraw
            }
            Dialog::None => Effect::Nothing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(label: &'static str, setting: Setting, values: &[&str]) -> Row {
        Row {
            label,
            setting,
            values: values.iter().map(|v| v.to_string()).collect(),
            selected: 0,
            initial: 0,
            help: "help",
        }
    }

    fn state() -> State {
        State::new(vec![
            row("Flavour", Setting::Flavour, &["Unicorn", "Sumo", "Ninja"]),
            row("Mascot", Setting::Mascot, &["Shown", "Hidden"]),
            row("Turbo", Setting::Turbo, &["On", "Off"]),
        ])
    }

    #[test]
    fn moving_down_past_the_last_row_wraps_to_the_first() {
        let mut s = state();
        for expected in [1, 2, 0] {
            assert_eq!(s.key(Key::Down), Effect::Redraw);
            assert_eq!(s.selected, expected);
        }
    }

    #[test]
    fn moving_up_from_the_first_row_wraps_to_the_last() {
        let mut s = state();
        s.key(Key::Up);
        assert_eq!(s.selected, 2);
    }

    #[test]
    fn changing_a_value_wraps_in_both_directions() {
        let mut s = state();
        assert_eq!(s.current().value(), "Unicorn");
        s.key(Key::Right);
        assert_eq!(s.current().value(), "Sumo");
        s.key(Key::Left);
        s.key(Key::Left);
        assert_eq!(
            s.current().value(),
            "Ninja",
            "left from the first should wrap"
        );
    }

    #[test]
    fn turbo_changes_on_screen_and_is_saved_like_any_other_row() {
        let mut s = state();
        s.selected = 2;
        s.key(Key::Right);
        assert_eq!(s.current().value(), "Off");
        assert!(s.current().changed());
        assert_eq!(s.changes(), vec![(Setting::Turbo, "Off")]);
        assert!(s.dirty());
    }

    #[test]
    fn escape_with_nothing_changed_leaves_at_once() {
        let mut s = state();
        assert_eq!(s.key(Key::Esc), Effect::Exit);
        assert_eq!(s.dialog, Dialog::None);
    }

    #[test]
    fn escape_with_changes_asks_first_and_n_goes_back() {
        let mut s = state();
        s.key(Key::Right);
        assert_eq!(s.key(Key::Esc), Effect::Redraw);
        assert_eq!(s.dialog, Dialog::Quit);
        assert_eq!(s.answer(false), Effect::Redraw);
        assert_eq!(s.dialog, Dialog::None, "N returns to the screen");
        assert!(s.dirty(), "and the change is still there");
    }

    #[test]
    fn the_quit_dialog_defaults_to_going_back_not_losing_the_changes() {
        let mut s = state();
        s.key(Key::Right);
        s.key(Key::Esc);
        assert_eq!(s.key(Key::Enter), Effect::Redraw);
        assert_eq!(s.dialog, Dialog::None);
    }

    #[test]
    fn quitting_with_y_leaves_without_saving() {
        let mut s = state();
        s.key(Key::Right);
        s.key(Key::Esc);
        assert_eq!(s.answer(true), Effect::Exit);
    }

    #[test]
    fn f10_then_y_saves_exactly_the_changed_settings() {
        let mut s = state();
        s.key(Key::Right); // Flavour -> Sumo
        s.selected = 2;
        s.key(Key::Right); // Turbo -> Off, saved too, just into a different file
        assert_eq!(s.key(Key::F10), Effect::Redraw);
        assert_eq!(s.dialog, Dialog::Save);
        assert_eq!(s.answer(true), Effect::Save);
        assert_eq!(
            s.changes(),
            vec![(Setting::Flavour, "Sumo"), (Setting::Turbo, "Off")]
        );
    }

    #[test]
    fn the_save_dialog_defaults_to_saving() {
        let mut s = state();
        s.key(Key::Right);
        s.key(Key::F10);
        assert_eq!(s.key(Key::Enter), Effect::Save);
    }

    #[test]
    fn escape_backs_out_of_the_save_dialog() {
        let mut s = state();
        s.key(Key::F10);
        assert_eq!(s.key(Key::Esc), Effect::Redraw);
        assert_eq!(s.dialog, Dialog::None);
    }

    #[test]
    fn enter_on_the_screen_previews() {
        let mut s = state();
        assert_eq!(s.key(Key::Enter), Effect::Preview);
    }

    #[test]
    fn ctrl_c_leaves_from_anywhere_without_asking() {
        for setup in [Dialog::None, Dialog::Save, Dialog::Quit] {
            let mut s = state();
            s.key(Key::Right);
            s.dialog = setup;
            assert_eq!(s.key(Key::CtrlC), Effect::Exit, "{setup:?}");
        }
    }

    #[test]
    fn a_row_with_one_value_cannot_move() {
        let mut s = State::new(vec![row("Theme", Setting::Theme, &["Unchanged"])]);
        s.key(Key::Right);
        assert_eq!(s.current().value(), "Unchanged");
        assert!(!s.current().changed());
    }

    #[test]
    fn returning_a_value_to_where_it_started_is_not_a_change() {
        let mut s = state();
        s.key(Key::Right);
        assert!(s.dirty());
        s.key(Key::Left);
        assert!(!s.dirty(), "back where it began is not a change to save");
    }
}
