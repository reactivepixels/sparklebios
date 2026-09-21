//! The screensaver's own state machine. Pure: no terminal, no clock, no randomness. A DVD-logo
//! bounce, reversing direction whenever the box it moves touches a wall.

/// The bouncing box: its position (top left corner, one indexed to match a terminal's own
/// cursor addressing), its size, and the terminal it bounces inside of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Model {
    /// The terminal's width in columns.
    pub cols: i32,
    /// The terminal's height in rows.
    pub rows: i32,
    /// The box's own width: 21 for the text wordmark, or the image's cell width in image mode.
    pub width: i32,
    /// The box's own height: 1 for the text wordmark, or the image's cell height in image mode.
    pub height: i32,
    /// The box's left edge, one indexed.
    pub x: i32,
    /// The box's top edge, one indexed.
    pub y: i32,
    /// Horizontal step per frame: 1 or -1.
    pub dx: i32,
    /// Vertical step per frame: 1 or -1.
    pub dy: i32,
}

impl Model {
    /// A fresh bounce starting near the top left corner, moving down and to the right, exactly
    /// as the proof of concept in `extras/ultra/ultra.zsh` began.
    pub fn new(cols: i32, rows: i32, width: i32, height: i32) -> Model {
        Model {
            cols,
            rows,
            width,
            height,
            x: 2,
            y: 2,
            dx: 1,
            dy: 1,
        }
    }

    /// Moves the box one frame, reversing whichever axis just touched a wall. A wall on one axis
    /// never affects the other: a screensaver that bounced both at once on a corner hit would
    /// stop covering the screen evenly over time.
    pub fn step(&mut self) {
        self.x += self.dx;
        self.y += self.dy;
        if self.x <= 1 || self.x + self.width >= self.cols {
            self.dx = -self.dx;
        }
        if self.y <= 1 || self.y + self.height >= self.rows {
            self.dy = -self.dy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_bounce_starts_near_the_top_left_moving_down_and_right() {
        let m = Model::new(80, 24, 21, 1);
        assert_eq!((m.x, m.y, m.dx, m.dy), (2, 2, 1, 1));
    }

    #[test]
    fn stepping_moves_the_box_by_its_current_direction() {
        let mut m = Model::new(80, 24, 21, 1);
        m.step();
        assert_eq!((m.x, m.y), (3, 3));
    }

    #[test]
    fn hitting_the_right_wall_reverses_the_horizontal_direction_only() {
        let mut m = Model::new(80, 24, 21, 1);
        m.x = 58; // one step from 59 + 21 >= 80
        let dy_before = m.dy;
        m.step();
        assert_eq!(m.x, 59);
        assert_eq!(m.dx, -1, "the horizontal direction should have reversed");
        assert_eq!(m.dy, dy_before, "the vertical direction is untouched");
    }

    #[test]
    fn hitting_the_left_wall_reverses_the_horizontal_direction() {
        let mut m = Model::new(80, 24, 21, 1);
        m.x = 2;
        m.dx = -1;
        m.step();
        assert_eq!(m.x, 1);
        assert_eq!(
            m.dx, 1,
            "bouncing off the left wall should turn it back around"
        );
    }

    #[test]
    fn hitting_the_bottom_wall_reverses_the_vertical_direction_only() {
        let mut m = Model::new(80, 24, 21, 1);
        m.y = 22;
        let dx_before = m.dx;
        m.step();
        assert_eq!(m.y, 23);
        assert_eq!(m.dy, -1);
        assert_eq!(m.dx, dx_before);
    }

    #[test]
    fn hitting_the_top_wall_reverses_the_vertical_direction() {
        let mut m = Model::new(80, 24, 21, 1);
        m.y = 2;
        m.dy = -1;
        m.step();
        assert_eq!(m.y, 1);
        assert_eq!(m.dy, 1);
    }

    #[test]
    fn a_taller_box_bounces_off_the_bottom_wall_sooner() {
        // An image mode box, five rows tall, should turn around five rows short of the floor.
        let mut m = Model::new(80, 24, 48, 5);
        m.y = 18; // 19 + 5 >= 24
        m.step();
        assert_eq!(m.y, 19);
        assert_eq!(m.dy, -1);
    }

    #[test]
    fn repeated_stepping_keeps_the_box_bouncing_back_and_forth_forever() {
        // A smoke test: run several hundred frames in a small terminal and confirm the box
        // never wanders outside it by more than the one cell a wall bounce can overshoot by.
        let mut m = Model::new(80, 24, 21, 1);
        for _ in 0..1000 {
            m.step();
            assert!(m.x >= 0 && m.x <= m.cols, "x escaped the terminal: {}", m.x);
            assert!(m.y >= 0 && m.y <= m.rows, "y escaped the terminal: {}", m.y);
        }
    }
}
