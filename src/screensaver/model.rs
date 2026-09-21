//! The screensaver's own state machine. Pure: no terminal, no clock. A DVD-logo bounce,
//! reversing direction whenever the box it moves touches a wall, plus the sparkle that rides
//! along with it: a tint or a ripple on every bounce, a twinkle beside the edge that was hit, and
//! an idle ripple if the box goes quiet for too long. The one impurity is `rng`, a seed advanced
//! by hand (see `next_random`) rather than read from the clock, so every decision here can be
//! replayed exactly in a test; only `mod.rs` ever seeds it from the real clock.

/// Which of the two screensaver renderers a `Model` is driving. The bounce, the tint and the
/// twinkle are identical either way; only a ripple's own length differs, so this is the one
/// thing `Model::new` needs told about the mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The real wordmark image, moved by placement: `RIPPLE_SEQUENCE`'s eight frames.
    Image,
    /// The six colour text fallback: seven frames of stepping one letter colour at a time.
    Text,
}

impl Kind {
    fn ripple_frames(self) -> u8 {
        match self {
            Kind::Image => 8,
            Kind::Text => 7,
        }
    }
}

/// One of the fourteen wordmark pictures this screen can show. Transmitted once each, at
/// startup, and never resent; see `sprites/wordmark/` and `mod.rs`'s `image_bytes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Image {
    /// The full colour wordmark. The steady image before the first tint, and always the last
    /// frame, and the one after it, of a ripple.
    Rainbow,
    /// A ripple's first transitional frame.
    Ripple1,
    /// A ripple's second transitional frame.
    Ripple2,
    /// A ripple's third transitional frame.
    Ripple3,
    /// A ripple's fourth transitional frame.
    Ripple4,
    /// A ripple's fifth transitional frame.
    Ripple5,
    /// A ripple's sixth and last transitional frame.
    Ripple6,
    /// The solid red tint.
    Red,
    /// The solid yellow tint.
    Yellow,
    /// The solid green tint.
    Green,
    /// The solid cyan tint.
    Cyan,
    /// The solid blue tint.
    Blue,
    /// The solid magenta tint.
    Magenta,
    /// The solid white tint.
    White,
}

impl Image {
    /// Every image this screen owns, in the fixed order `mod.rs` transmits and ids them by: an
    /// image's position here is its offset from `mod.rs`'s `IMAGE_ID_BASE`.
    pub const ALL: [Image; 14] = [
        Image::Rainbow,
        Image::Ripple1,
        Image::Ripple2,
        Image::Ripple3,
        Image::Ripple4,
        Image::Ripple5,
        Image::Ripple6,
        Image::Red,
        Image::Yellow,
        Image::Green,
        Image::Cyan,
        Image::Blue,
        Image::Magenta,
        Image::White,
    ];

    /// This image's position in `ALL`, and so its offset from `mod.rs`'s `IMAGE_ID_BASE`.
    pub fn offset(self) -> u32 {
        Self::ALL
            .iter()
            .position(|&i| i == self)
            .expect("every Image variant appears once in ALL") as u32
    }
}

/// The seven solid tints a bounce may switch to. Distinct from `Image::ALL`: `Image::Rainbow`
/// and the six `Image::Ripple*` frames are never a tint.
const TINTS: [Image; 7] = [
    Image::Red,
    Image::Yellow,
    Image::Green,
    Image::Cyan,
    Image::Blue,
    Image::Magenta,
    Image::White,
];

/// The eight frames a ripple plays, in order, one a frame: rainbow, the six ripple frames, then
/// rainbow again. `Model::ripple` is an index into this; see `current_image`.
pub const RIPPLE_SEQUENCE: [Image; 8] = [
    Image::Rainbow,
    Image::Ripple1,
    Image::Ripple2,
    Image::Ripple3,
    Image::Ripple4,
    Image::Ripple5,
    Image::Ripple6,
    Image::Rainbow,
];

/// How many frames a twinkle shows a glyph for, cycling `* + .`, before it clears.
const TWINKLE_FRAMES: u8 = 3;

/// About eight seconds with no bounce, at the roughly fifteen frames a second `mod.rs`'s
/// `FRAME_MS` drives this screen at: `8000 / 66 ≈ 121`. Forces a single ripple so the screen
/// never sits perfectly still.
pub const IDLE_RIPPLE_FRAMES: i32 = 121;

/// A twinkle in progress: the three cells it lights, in the model's own one indexed
/// `(column, row)` coordinates, and which of its three frames (`0`, `1` or `2`) is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Twinkle {
    /// The three cells it lights, as `(column, row)`.
    pub cells: [(i32, i32); 3],
    /// Which of its three frames (`0`, `1` or `2`) is showing.
    pub frame: u8,
}

/// Which wall(s) a step just touched. Both at once is a corner.
#[derive(Debug, Clone, Copy, Default)]
struct Hit {
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
}

impl Hit {
    fn horizontal(self) -> bool {
        self.left || self.right
    }

    fn vertical(self) -> bool {
        self.top || self.bottom
    }

    fn any(self) -> bool {
        self.horizontal() || self.vertical()
    }

    fn corner(self) -> bool {
        self.horizontal() && self.vertical()
    }
}

/// The bouncing box: its position (top left corner, one indexed to match a terminal's own
/// cursor addressing), its size, the terminal it bounces inside of, and the sparkle riding
/// along with it.
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
    /// The image shown when nothing is rippling: `Image::Rainbow` until the first bounce picks
    /// a tint, one of `TINTS` after that. A ripple leaves this alone until its own last frame
    /// resets it to `Image::Rainbow`; see `current_image`.
    pub tint: Image,
    /// The ripple in progress, as a frame index counting up from zero, or `None` between
    /// ripples. How many frames a ripple gets is fixed at construction; see `Kind`.
    pub ripple: Option<u8>,
    /// The twinkle in progress, or `None` between bounces.
    pub twinkle: Option<Twinkle>,
    /// Frames since the last wall bounce; a bounce resets it to zero. Forces a ripple at
    /// `IDLE_RIPPLE_FRAMES`.
    idle_frames: i32,
    /// How long this screen's own ripple lasts, from `Kind::ripple_frames`.
    ripple_frames: u8,
    /// The pseudo random state behind every sparkle decision: which tint to switch to, and
    /// whether a bounce plays a ripple. Advanced one step, via `next_random`, each time a
    /// decision is needed. `mod.rs` seeds a real run from the clock, the same way
    /// `boot::seed_from_time` seeds the quip rotation; a test hands it a fixed number instead.
    rng: u64,
}

impl Model {
    /// A fresh bounce starting near the top left corner, moving down and to the right, exactly
    /// as the proof of concept in `extras/ultra/ultra.zsh` began, showing `Image::Rainbow` and
    /// sparkling nothing yet.
    pub fn new(cols: i32, rows: i32, width: i32, height: i32, kind: Kind, seed: u64) -> Model {
        Model {
            cols,
            rows,
            width,
            height,
            x: 2,
            y: 2,
            dx: 1,
            dy: 1,
            tint: Image::Rainbow,
            ripple: None,
            twinkle: None,
            idle_frames: 0,
            ripple_frames: kind.ripple_frames(),
            rng: seed,
        }
    }

    /// The image mode picture on screen right now: a `RIPPLE_SEQUENCE` frame mid ripple, `tint`
    /// otherwise. Text mode never calls this; see `ripple_color_offset`.
    pub fn current_image(&self) -> Image {
        match self.ripple {
            Some(frame) => RIPPLE_SEQUENCE[frame as usize],
            None => self.tint,
        }
    }

    /// How many steps a ripple has rotated the text mode letter colours by right now: `1` on a
    /// ripple's first frame, up through `Kind::Text`'s seven, or `None` between ripples. Image
    /// mode never calls this; see `current_image`.
    pub fn ripple_color_offset(&self) -> Option<u8> {
        self.ripple.map(|frame| frame + 1)
    }

    /// Moves the box one frame, reversing whichever axis just touched a wall. A wall on one axis
    /// never affects the other: a screensaver that bounced both at once on a corner hit would
    /// stop covering the screen evenly over time.
    ///
    /// Every wall touch also sparkles: a corner always plays a ripple, any other wall plays one
    /// 70 percent of the time and a tint switch the rest, and either way a twinkle lights up
    /// beside whichever edge was hit. Going quiet for `IDLE_RIPPLE_FRAMES` plays one ripple with
    /// no twinkle, since nothing was actually hit.
    pub fn step(&mut self) {
        self.x += self.dx;
        self.y += self.dy;

        let mut hit = Hit::default();
        if self.x <= 1 {
            hit.left = true;
        }
        if self.x + self.width >= self.cols {
            hit.right = true;
        }
        if self.y <= 1 {
            hit.top = true;
        }
        if self.y + self.height >= self.rows {
            hit.bottom = true;
        }
        if hit.horizontal() {
            self.dx = -self.dx;
        }
        if hit.vertical() {
            self.dy = -self.dy;
        }

        if hit.any() {
            self.idle_frames = 0;
            self.twinkle = Some(Twinkle {
                cells: self.twinkle_cells(hit),
                frame: 0,
            });
            if hit.corner() || self.next_random() % 10 < 3 {
                self.ripple = Some(0);
            } else {
                self.ripple = None;
                self.tint = self.random_tint_excluding(self.tint);
            }
        } else {
            self.idle_frames += 1;
            self.advance_twinkle();
            if self.idle_frames >= IDLE_RIPPLE_FRAMES {
                self.idle_frames = 0;
                self.ripple = Some(0);
            } else {
                self.advance_ripple();
            }
        }
    }

    /// The three cells a twinkle lights for the wall(s) `hit` just touched: outside the hit
    /// edge, spread across the box's own centre for a single wall, or the diagonal corner cell
    /// plus its two straight neighbours for a corner. Every cell here needs only the one cell of
    /// margin a wall bounce already overshoots by (see `step`'s own doc comment and the
    /// `repeated_stepping...` test below), so a twinkle never reaches for a cell the box's own
    /// bounce has not already guaranteed is on screen.
    fn twinkle_cells(&self, hit: Hit) -> [(i32, i32); 3] {
        let left = self.x;
        let right = self.x + self.width - 1;
        let top = self.y;
        let bottom = self.y + self.height - 1;
        let mid_row = self.y + self.height / 2;
        let mid_col = self.x + self.width / 2;
        let outer_col = if hit.left { left - 1 } else { right + 1 };
        let outer_row = if hit.top { top - 1 } else { bottom + 1 };

        match (hit.horizontal(), hit.vertical()) {
            (true, false) => [
                (outer_col, mid_row - 1),
                (outer_col, mid_row),
                (outer_col, mid_row + 1),
            ],
            (false, true) => [
                (mid_col - 1, outer_row),
                (mid_col, outer_row),
                (mid_col + 1, outer_row),
            ],
            (true, true) => {
                let edge_row = if hit.top { top } else { bottom };
                let edge_col = if hit.left { left } else { right };
                [
                    (outer_col, outer_row),
                    (outer_col, edge_row),
                    (edge_col, outer_row),
                ]
            }
            (false, false) => unreachable!("twinkle_cells is only called once a wall was hit"),
        }
    }

    /// Ages a twinkle already in progress by one frame, clearing it once it has shown all
    /// `TWINKLE_FRAMES`. Never called on the same frame a bounce starts a fresh one.
    fn advance_twinkle(&mut self) {
        if let Some(t) = &mut self.twinkle {
            t.frame += 1;
            if t.frame >= TWINKLE_FRAMES {
                self.twinkle = None;
            }
        }
    }

    /// Ages a ripple already in progress by one frame, ending it and resetting `tint` to
    /// `Image::Rainbow` once it has shown all `ripple_frames`. Never called on the same frame a
    /// bounce or the idle timeout starts a fresh one.
    fn advance_ripple(&mut self) {
        if let Some(frame) = self.ripple {
            let next = frame + 1;
            if next >= self.ripple_frames {
                self.ripple = None;
                self.tint = Image::Rainbow;
            } else {
                self.ripple = Some(next);
            }
        }
    }

    /// A tint drawn from `TINTS`, never `current`: the guarantee behind "a tint is never chosen
    /// twice in a row" (see the test below). When `current` is not itself a tint (`Rainbow` or a
    /// ripple frame), nothing is excluded and all seven are candidates.
    ///
    /// `pub(super)` rather than private: `view`'s own text mode test reaches for this directly,
    /// to check its own letter colouring never repeats either, off the very same draw this
    /// drives for image mode (see `view::tint_letter_styles`).
    pub(super) fn random_tint_excluding(&mut self, current: Image) -> Image {
        let pool: Vec<Image> = TINTS.into_iter().filter(|&t| t != current).collect();
        let index = (self.next_random() as usize) % pool.len();
        pool[index]
    }

    /// splitmix64, run by hand so this crate never needs a `rand` dependency: a small, well
    /// known generator with no all-zero fixed point to dodge, so any seed, including zero,
    /// produces a full length sequence. Advances `rng` and returns the next value; never read
    /// directly.
    fn next_random(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mechanics tests below only care about the bounce, not the sparkle, so the mode and
    /// seed are arbitrary; `Kind::Image` and a fixed seed keep them deterministic.
    fn bounce_model(cols: i32, rows: i32, width: i32, height: i32) -> Model {
        Model::new(cols, rows, width, height, Kind::Image, 1)
    }

    #[test]
    fn a_fresh_bounce_starts_near_the_top_left_moving_down_and_right() {
        let m = bounce_model(80, 24, 21, 1);
        assert_eq!((m.x, m.y, m.dx, m.dy), (2, 2, 1, 1));
        assert_eq!(m.tint, Image::Rainbow);
        assert_eq!(m.ripple, None);
        assert_eq!(m.twinkle, None);
    }

    #[test]
    fn stepping_moves_the_box_by_its_current_direction() {
        let mut m = bounce_model(80, 24, 21, 1);
        m.step();
        assert_eq!((m.x, m.y), (3, 3));
    }

    #[test]
    fn hitting_the_right_wall_reverses_the_horizontal_direction_only() {
        let mut m = bounce_model(80, 24, 21, 1);
        m.x = 58; // one step from 59 + 21 >= 80
        let dy_before = m.dy;
        m.step();
        assert_eq!(m.x, 59);
        assert_eq!(m.dx, -1, "the horizontal direction should have reversed");
        assert_eq!(m.dy, dy_before, "the vertical direction is untouched");
    }

    #[test]
    fn hitting_the_left_wall_reverses_the_horizontal_direction() {
        let mut m = bounce_model(80, 24, 21, 1);
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
        let mut m = bounce_model(80, 24, 21, 1);
        m.y = 22;
        let dx_before = m.dx;
        m.step();
        assert_eq!(m.y, 23);
        assert_eq!(m.dy, -1);
        assert_eq!(m.dx, dx_before);
    }

    #[test]
    fn hitting_the_top_wall_reverses_the_vertical_direction() {
        let mut m = bounce_model(80, 24, 21, 1);
        m.y = 2;
        m.dy = -1;
        m.step();
        assert_eq!(m.y, 1);
        assert_eq!(m.dy, 1);
    }

    #[test]
    fn a_taller_box_bounces_off_the_bottom_wall_sooner() {
        // An image mode box, five rows tall, should turn around five rows short of the floor.
        let mut m = bounce_model(80, 24, 48, 5);
        m.y = 18; // 19 + 5 >= 24
        m.step();
        assert_eq!(m.y, 19);
        assert_eq!(m.dy, -1);
    }

    #[test]
    fn repeated_stepping_keeps_the_box_bouncing_back_and_forth_forever() {
        // A smoke test: run several hundred frames in a small terminal and confirm the box
        // never wanders outside it by more than the one cell a wall bounce can overshoot by.
        let mut m = bounce_model(80, 24, 21, 1);
        for _ in 0..1000 {
            m.step();
            assert!(m.x >= 0 && m.x <= m.cols, "x escaped the terminal: {}", m.x);
            assert!(m.y >= 0 && m.y <= m.rows, "y escaped the terminal: {}", m.y);
        }
    }

    #[test]
    fn a_tint_is_never_chosen_twice_in_a_row() {
        // Every image as the "current" one, including the ones that are never themselves a
        // tint, over many draws each: `random_tint_excluding` must never hand back `current`.
        let mut m = bounce_model(80, 24, 48, 5);
        for current in Image::ALL {
            for _ in 0..50 {
                let next = m.random_tint_excluding(current);
                assert_ne!(next, current, "picked the tint that was already showing");
            }
        }
    }

    #[test]
    fn a_ripple_plays_exactly_the_eight_images_in_order() {
        // A corner hit: x and y both about to touch a wall on the same step, which always
        // ripples regardless of the dice, so this needs no particular seed.
        let mut m = Model::new(80, 24, 48, 5, Kind::Image, 42);
        m.x = 31; // one step from 32 + 48 >= 80
        m.y = 18; // one step from 19 + 5 >= 24
        let mut seen = Vec::new();
        for _ in 0..8 {
            m.step();
            seen.push(m.current_image());
        }
        assert_eq!(seen, RIPPLE_SEQUENCE.to_vec());
        // The ninth frame is back to steady, and steady is rainbow: a ripple always ends there,
        // whatever tint was showing before it started.
        m.step();
        assert_eq!(m.ripple, None);
        assert_eq!(m.tint, Image::Rainbow);
        assert_eq!(m.current_image(), Image::Rainbow);
    }

    #[test]
    fn a_twinkle_lasts_three_frames_and_its_cells_fall_outside_the_image_bounds() {
        let mut m = bounce_model(80, 24, 21, 1);
        m.x = 58; // one step from hitting the right wall, same as the bounce test above
        m.step();
        let outside = |(col, row): (i32, i32)| {
            col < m.x || col > m.x + m.width - 1 || row < m.y || row > m.y + m.height - 1
        };
        let twinkle = m.twinkle.expect("a bounce should start a twinkle");
        assert_eq!(twinkle.frame, 0);
        assert!(
            twinkle.cells.iter().all(|&c| outside(c)),
            "a twinkle cell fell inside the box: {:?}",
            twinkle.cells
        );

        m.step();
        assert_eq!(m.twinkle.expect("still twinkling").frame, 1);
        m.step();
        assert_eq!(m.twinkle.expect("still twinkling").frame, 2);
        m.step();
        assert_eq!(m.twinkle, None, "a twinkle should clear after three frames");
    }

    #[test]
    fn eight_seconds_without_a_bounce_forces_a_single_ripple() {
        // A field far too big for the box to reach a wall in over this many frames, so every
        // step below is idle.
        let mut m = Model::new(200, 200, 21, 1, Kind::Text, 9);
        for _ in 0..IDLE_RIPPLE_FRAMES - 1 {
            m.step();
        }
        assert_eq!(m.ripple, None, "should not have rippled yet");
        m.step();
        assert_eq!(m.ripple, Some(0), "the idle timeout should force a ripple");
    }
}
