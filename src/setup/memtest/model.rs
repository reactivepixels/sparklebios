//! The Memory Test easter egg: a small Breakout, pure. No terminal, no clock, no randomness
//! beyond the seed the caller supplies once, at construction. Everything the game does in
//! response to a key or a tick is decided here, so it can all be tested without a tty.

/// A key the game understands. Anything else is ignored by the caller before it ever reaches
/// here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Move the paddle left.
    Left,
    /// Move the paddle right.
    Right,
    /// Launch the ball, when it is resting on the paddle.
    Space,
    /// Give up. The game ends as a loss and whatever was cleared still counts.
    Esc,
    /// Walk away. Nothing is recorded, which is what Ctrl-C means everywhere else.
    CtrlC,
}

/// What the caller should do after a key or a tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Nothing worth redrawing happened.
    Nothing,
    /// The game changed and should be redrawn.
    Redraw,
    /// The game has just reached its one true ending: every ball lost, or every brick cleared.
    /// This is the single moment the result should be saved. Giving up never produces this.
    Ended,
    /// Leave the game and return to setup. Nothing is saved.
    Exit,
}

/// Columns subtracted from the terminal's width to get the field's inner width: a one column
/// margin and a frame character, on each side.
pub const FIELD_WIDTH_MARGIN: usize = 4;
/// Rows subtracted from the terminal's height to get the field's inner height: the header, the
/// top frame, the bottom frame, the help line, and one spare row.
pub const FIELD_HEIGHT_MARGIN: usize = 5;
/// How many rows of bricks the field starts with.
pub const BRICK_ROWS: usize = 5;
/// How many bricks make up one row.
pub const BRICKS_PER_ROW: usize = 12;
/// How many bricks the field starts with in total.
pub const TOTAL_BRICKS: usize = BRICK_ROWS * BRICKS_PER_ROW;
/// The paddle's width, in field columns.
pub const PADDLE_WIDTH: usize = 10;

/// Brick row colours, top to bottom: red, yellow, green, cyan, blue. The rainbow order used
/// elsewhere, as plain ANSI SGR foreground codes.
pub const BRICK_COLOURS: [u8; BRICK_ROWS] = [31, 33, 32, 36, 34];

const PADDLE_STEP: i32 = 3;
const BALLS_PER_GAME: u8 = 3;
/// Ticks a second: `mod.rs` drives the game at roughly this rate. Named here, not read from
/// there, so the per tick step below is derived, not guessed.
const TICKS_PER_SECOND: f64 = 30.0;
/// Seconds the ball takes to fall from the lowest brick row to the bat, at the game's starting
/// speed, at every terminal size. This, not a fixed speed, is the constant: a taller field gives
/// the ball more open rows to fall through, so its starting speed scales with the field rather
/// than its time of flight growing with it.
const FALL_SECONDS: f64 = 1.5;

/// The ball's base vertical and horizontal step, in field cells a tick, before the speed up.
/// Worked out from `field_rows` rather than a `const`, since the rate is a function of the field:
/// the open rows between the lowest brick row and the bat are `field_rows - BRICK_ROWS`, crossed
/// in `FALL_SECONDS` however many of them a taller terminal gives the ball. A terminal cell is
/// roughly twice as tall as it is wide, so the horizontal rate is always twice the vertical one:
/// the ball still traces a true forty five degree diagonal on screen, at any size. The horizontal
/// step is also the most sideways a paddle bounce can send the ball: an edge hit uses the whole
/// of it, a centre hit none.
fn base_steps(field_rows: usize) -> (f64, f64) {
    let open_rows = (field_rows - BRICK_ROWS) as f64;
    let vertical_per_second = open_rows / FALL_SECONDS;
    let horizontal_per_second = vertical_per_second * 2.0;
    (
        vertical_per_second / TICKS_PER_SECOND,
        horizontal_per_second / TICKS_PER_SECOND,
    )
}

/// How much faster the ball gets for every ten bricks broken.
const SPEED_UP_FACTOR: f64 = 1.05;
/// The ball is never faster than this multiple of its starting speed, however many bricks are
/// cleared.
const SPEED_CAP: f64 = 1.5;
/// One second at the tick rate: how long the header holds `FAIL` before the result screen.
const FAIL_PAUSE_TICKS: u8 = TICKS_PER_SECOND as u8;

/// One brick on the field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Brick {
    /// Whether this brick has yet to be broken.
    pub alive: bool,
    /// How many K clearing this brick counts toward the score.
    pub kb: u64,
}

/// Which part of a game is showing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    /// The ball rests on the paddle, waiting for Space.
    Ready,
    /// The ball is in motion.
    Playing,
    /// The header holds `FAIL` for a beat before the result screen appears.
    FailPause {
        /// Ticks left before the result screen appears.
        ticks_left: u8,
    },
    /// The result screen: waiting for any key to leave.
    Result {
        /// Whether every brick was cleared.
        won: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
/// One game of the Memory Test easter egg, and everything it needs to keep playing.
pub struct Game {
    /// Every brick on the field, in row major order.
    pub bricks: Vec<Brick>,
    /// The field's inner width in columns, fixed for the life of the game: the terminal's width
    /// at construction, minus `FIELD_WIDTH_MARGIN`.
    pub field_cols: usize,
    /// The field's inner height in rows, fixed for the life of the game: the terminal's height at
    /// construction, minus `FIELD_HEIGHT_MARGIN`.
    pub field_rows: usize,
    /// The paddle's leftmost column.
    pub paddle_col: usize,
    /// The ball's horizontal position, in field cells.
    pub ball_x: f64,
    /// The ball's vertical position, in field cells.
    pub ball_y: f64,
    /// The ball's horizontal step per tick.
    pub ball_dx: f64,
    /// The ball's vertical step per tick.
    pub ball_dy: f64,
    /// Balls left before the game is lost.
    pub balls_left: u8,
    /// The machine's total memory, in K: the game's target score.
    pub mem_kb: u64,
    /// K left to clear to win.
    pub remaining_kb: u64,
    /// How many bricks have been broken so far.
    pub bricks_broken: u32,
    /// The ball's current speed multiplier over its base speed.
    pub speed: f64,
    /// Which part of the game is showing.
    pub phase: Phase,
    /// The best K ever cleared before this game started. Never changed by play; the result
    /// screen compares this game's score against it to decide whether to say `New record.`
    pub best_kb: u64,
    /// Whether any game has ever been fully cleared, including this one from the moment it is.
    pub cleared: bool,
    seed: u64,
}

impl Game {
    /// A fresh game over `mem_kb` of memory, on a terminal `term_cols` by `term_rows`. `best_kb`
    /// and `cleared_before` come from `state.json`. `seed` decides the launch angle
    /// deterministically; the model never generates randomness of its own.
    pub fn new(
        mem_kb: u64,
        best_kb: u64,
        cleared_before: bool,
        seed: u64,
        term_cols: u16,
        term_rows: u16,
    ) -> Game {
        let field_cols = (term_cols as usize).saturating_sub(FIELD_WIDTH_MARGIN);
        let field_rows = (term_rows as usize).saturating_sub(FIELD_HEIGHT_MARGIN);
        let paddle_col = (field_cols - PADDLE_WIDTH) / 2;
        let mut game = Game {
            bricks: build_bricks(mem_kb),
            field_cols,
            field_rows,
            paddle_col,
            ball_x: 0.0,
            ball_y: 0.0,
            ball_dx: 0.0,
            ball_dy: 0.0,
            balls_left: BALLS_PER_GAME,
            mem_kb,
            remaining_kb: mem_kb,
            bricks_broken: 0,
            speed: 1.0,
            phase: Phase::Ready,
            best_kb,
            cleared: cleared_before,
            seed,
        };
        game.attach_ball();
        game
    }

    /// Memory cleared so far, in K.
    pub fn score(&self) -> u64 {
        self.mem_kb - self.remaining_kb
    }

    /// Whether this game's score beats the best that stood when it started.
    pub fn is_new_record(&self) -> bool {
        self.score() > self.best_kb
    }

    /// Applies one key press, returning what the caller should do next.
    pub fn key(&mut self, key: Key) -> Effect {
        match self.phase {
            Phase::Result { .. } => Effect::Exit,
            Phase::FailPause { .. } => match key {
                Key::CtrlC => Effect::Exit,
                Key::Esc => {
                    self.phase = Phase::Result { won: false };
                    Effect::Ended
                }
                _ => Effect::Nothing,
            },
            Phase::Ready | Phase::Playing => match key {
                Key::CtrlC => Effect::Exit,
                // Giving up is still a result. Whatever was cleared was cleared, so the game
                // ends the same way losing the last ball does: the result screen, and the score
                // saved if it is a best.
                Key::Esc => {
                    self.phase = Phase::Result { won: false };
                    Effect::Ended
                }
                Key::Left => {
                    self.move_paddle(-1);
                    Effect::Redraw
                }
                Key::Right => {
                    self.move_paddle(1);
                    Effect::Redraw
                }
                Key::Space => {
                    if self.phase == Phase::Ready {
                        self.launch();
                        Effect::Redraw
                    } else {
                        Effect::Nothing
                    }
                }
            },
        }
    }

    /// Advances the game by one tick, returning what the caller should do next.
    pub fn tick(&mut self) -> Effect {
        match self.phase {
            Phase::Result { .. } => Effect::Nothing,
            Phase::FailPause { ticks_left } => {
                if ticks_left <= 1 {
                    self.phase = Phase::Result { won: false };
                    Effect::Ended
                } else {
                    self.phase = Phase::FailPause {
                        ticks_left: ticks_left - 1,
                    };
                    Effect::Nothing
                }
            }
            Phase::Ready => Effect::Nothing,
            Phase::Playing => self.advance_ball(),
        }
    }

    fn move_paddle(&mut self, direction: i32) {
        let max_col = (self.field_cols - PADDLE_WIDTH) as i32;
        let step = PADDLE_STEP * direction;
        let new_col = (self.paddle_col as i32 + step).clamp(0, max_col);
        self.paddle_col = new_col as usize;
        if self.phase == Phase::Ready {
            self.place_ball_on_paddle();
        }
    }

    fn place_ball_on_paddle(&mut self) {
        self.ball_x = (self.paddle_col + PADDLE_WIDTH / 2) as f64;
        self.ball_y = (self.field_rows - 1 - 2) as f64;
    }

    fn attach_ball(&mut self) {
        self.phase = Phase::Ready;
        self.ball_dx = 0.0;
        self.ball_dy = 0.0;
        self.place_ball_on_paddle();
    }

    fn launch(&mut self) {
        self.phase = Phase::Playing;
        let direction = if self.seed % 2 == 0 { -1.0 } else { 1.0 };
        let (base_vertical_step, base_horizontal_step) = base_steps(self.field_rows);
        self.ball_dx = base_horizontal_step * self.speed * direction;
        self.ball_dy = -base_vertical_step * self.speed;
    }

    /// The horizontal speed a paddle bounce sends the ball off at, given where along the paddle
    /// `hit_x` struck. The centre sends it straight up; the edges send it out at up to the base
    /// horizontal step, so the player can aim the return. The vertical speed is untouched by
    /// this: only `advance_ball` flips its sign, so the ball never gets shallower than it already
    /// was, however wide the bounce.
    fn paddle_bounce_dx(&self, hit_x: f64) -> f64 {
        let half_width = PADDLE_WIDTH as f64 / 2.0;
        let centre = self.paddle_col as f64 + half_width;
        let offset = ((hit_x - centre) / half_width).clamp(-1.0, 1.0);
        let (_, base_horizontal_step) = base_steps(self.field_rows);
        offset * base_horizontal_step * self.speed
    }

    /// One tick of ball physics: walls, the paddle, and bricks, in that order. Returns the effect
    /// for the caller, same as `key`.
    fn advance_ball(&mut self) -> Effect {
        let mut next_x = self.ball_x + self.ball_dx;
        if next_x < 0.0 || next_x > (self.field_cols - 1) as f64 {
            self.ball_dx = -self.ball_dx;
            next_x = self.ball_x + self.ball_dx;
        }

        let mut next_y = self.ball_y + self.ball_dy;
        if next_y < 0.0 {
            self.ball_dy = -self.ball_dy;
            next_y = self.ball_y + self.ball_dy;
        }

        let paddle_row = (self.field_rows - 1) as f64;
        if next_y >= paddle_row {
            let paddle_left = self.paddle_col as f64;
            let paddle_right = paddle_left + PADDLE_WIDTH as f64;
            if next_x >= paddle_left && next_x < paddle_right {
                self.ball_dx = self.paddle_bounce_dx(next_x);
                self.ball_dy = -self.ball_dy;
                next_y = paddle_row - 1.0;
            } else {
                return self.lose_ball();
            }
        } else if let Some(effect) = self.hit_brick(next_x, next_y) {
            return effect;
        }

        self.ball_x = next_x;
        self.ball_y = next_y;
        Effect::Redraw
    }

    /// Destroys the brick at `(x, y)`, if any is alive there, and reports what happened. `None`
    /// when there is no brick to hit, meaning the ball keeps travelling.
    fn hit_brick(&mut self, x: f64, y: f64) -> Option<Effect> {
        let row = y.round();
        if row < 0.0 || row >= BRICK_ROWS as f64 {
            return None;
        }
        let col = brick_col_at(self.field_cols, x)?;
        let idx = row as usize * BRICKS_PER_ROW + col;
        let brick = self.bricks.get_mut(idx)?;
        if !brick.alive {
            return None;
        }

        brick.alive = false;
        self.remaining_kb -= brick.kb;
        self.bricks_broken += 1;
        self.apply_speed_up_if_milestone();
        self.ball_dy = -self.ball_dy;
        self.ball_x = x;
        self.ball_y = y;

        if self.remaining_kb == 0 {
            self.phase = Phase::Result { won: true };
            self.cleared = true;
            Some(Effect::Ended)
        } else {
            Some(Effect::Redraw)
        }
    }

    /// Every ten bricks, the ball gets `SPEED_UP_FACTOR` faster, up to `SPEED_CAP` times its
    /// starting speed and never beyond it, however many bricks that ten was part of clearing.
    fn apply_speed_up_if_milestone(&mut self) {
        if self.bricks_broken % 10 != 0 || self.speed >= SPEED_CAP {
            return;
        }
        let new_speed = (self.speed * SPEED_UP_FACTOR).min(SPEED_CAP);
        let factor = new_speed / self.speed;
        self.speed = new_speed;
        self.ball_dx *= factor;
        self.ball_dy *= factor;
    }

    fn lose_ball(&mut self) -> Effect {
        if self.balls_left > 1 {
            self.balls_left -= 1;
            self.attach_ball();
        } else {
            self.balls_left = 0;
            self.phase = Phase::FailPause {
                ticks_left: FAIL_PAUSE_TICKS,
            };
        }
        Effect::Redraw
    }
}

/// The width, in columns, of each of the twelve bricks in a row, tiling `field_cols` exactly. The
/// base width is `field_cols / BRICKS_PER_ROW`; the leftover columns go one each to the first few
/// bricks, so no two bricks in a row ever differ by more than one column. This is a separate sum
/// from the sixty bricks' K amounts below: that one dumps its remainder on the last brick, where
/// it is invisible; doing the same here would leave one brick visibly wider than the rest.
pub fn brick_widths(field_cols: usize) -> [usize; BRICKS_PER_ROW] {
    let base = field_cols / BRICKS_PER_ROW;
    let remainder = field_cols % BRICKS_PER_ROW;
    let mut widths = [base; BRICKS_PER_ROW];
    for width in widths.iter_mut().take(remainder) {
        *width += 1;
    }
    widths
}

/// Which of the twelve bricks in a row `x` falls under, tiling `field_cols` the same way
/// `brick_widths` describes. `None` only when `field_cols` is zero, which never happens once
/// `Game::new` has run on a terminal setup already refused to accept as too small.
fn brick_col_at(field_cols: usize, x: f64) -> Option<usize> {
    let target = x.round().clamp(0.0, field_cols.saturating_sub(1) as f64) as usize;
    let mut end = 0usize;
    for (col, width) in brick_widths(field_cols).iter().enumerate() {
        end += width;
        if target < end {
            return Some(col);
        }
    }
    None
}

/// `mem_kb / TOTAL_BRICKS`, integer division, with the remainder added to the last brick so the
/// sixty bricks always sum to exactly `mem_kb`.
fn build_bricks(mem_kb: u64) -> Vec<Brick> {
    let base = mem_kb / TOTAL_BRICKS as u64;
    let remainder = mem_kb % TOTAL_BRICKS as u64;
    (0..TOTAL_BRICKS)
        .map(|i| Brick {
            alive: true,
            kb: if i == TOTAL_BRICKS - 1 {
                base + remainder
            } else {
                base
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Matches what `setup::run` refuses to start the whole utility below, so it is what every
    /// test that does not care about a specific size should build a game at.
    const COLS: u16 = 80;
    const ROWS: u16 = 24;

    fn kill_all_bricks(game: &mut Game) {
        for brick in &mut game.bricks {
            brick.alive = false;
        }
    }

    #[test]
    fn sixty_bricks_sum_to_exactly_mem_kb_even_when_it_does_not_divide_by_sixty() {
        for mem_kb in [37748736u64, 8388608, 100, 59, 61, 1, 0, 12345] {
            let bricks = build_bricks(mem_kb);
            assert_eq!(bricks.len(), TOTAL_BRICKS);
            let sum: u64 = bricks.iter().map(|b| b.kb).sum();
            assert_eq!(sum, mem_kb, "mem_kb={mem_kb}");
        }
    }

    #[test]
    fn the_remainder_lands_on_the_last_brick() {
        let bricks = build_bricks(61);
        assert_eq!(bricks[TOTAL_BRICKS - 1].kb, 1 + 1);
        for b in &bricks[..TOTAL_BRICKS - 1] {
            assert_eq!(b.kb, 1);
        }
    }

    #[test]
    fn bricks_tile_the_row_exactly_at_several_terminal_widths() {
        for cols in [80usize, 120, 160] {
            let field_cols = cols - FIELD_WIDTH_MARGIN;
            let widths = brick_widths(field_cols);
            let sum: usize = widths.iter().sum();
            assert_eq!(sum, field_cols, "cols={cols}");
            let min = *widths.iter().min().unwrap();
            let max = *widths.iter().max().unwrap();
            assert!(max - min <= 1, "cols={cols} widths={widths:?}");
        }
    }

    #[test]
    fn eighty_columns_gives_four_bricks_of_seven_and_eight_of_six() {
        let widths = brick_widths(80 - FIELD_WIDTH_MARGIN);
        assert_eq!(widths.iter().filter(|&&w| w == 7).count(), 4);
        assert_eq!(widths.iter().filter(|&&w| w == 6).count(), 8);
    }

    #[test]
    fn the_field_fills_the_terminal_in_both_directions() {
        let g = Game::new(100, 0, false, 1, COLS, ROWS);
        assert_eq!(g.field_cols, 76);
        assert_eq!(g.field_rows, 19);

        let bigger = Game::new(100, 0, false, 1, 120, 40);
        assert_eq!(bigger.field_cols, 116);
        assert_eq!(bigger.field_rows, 35);
    }

    #[test]
    fn the_ball_bounces_off_the_left_wall() {
        let mut g = Game::new(100, 0, false, 1, COLS, ROWS);
        kill_all_bricks(&mut g);
        g.phase = Phase::Playing;
        g.ball_x = 0.2;
        g.ball_y = 5.0;
        g.ball_dx = -0.5;
        g.ball_dy = 0.0;
        g.tick();
        assert!(g.ball_dx > 0.0, "should have reflected off the left wall");
    }

    #[test]
    fn the_ball_bounces_off_the_right_wall() {
        let mut g = Game::new(100, 0, false, 1, COLS, ROWS);
        kill_all_bricks(&mut g);
        g.phase = Phase::Playing;
        g.ball_x = (g.field_cols - 1) as f64 - 0.2;
        g.ball_y = 5.0;
        g.ball_dx = 0.5;
        g.ball_dy = 0.0;
        g.tick();
        assert!(g.ball_dx < 0.0, "should have reflected off the right wall");
    }

    #[test]
    fn the_ball_bounces_off_the_ceiling() {
        let mut g = Game::new(100, 0, false, 1, COLS, ROWS);
        kill_all_bricks(&mut g);
        g.phase = Phase::Playing;
        g.ball_x = 36.0;
        g.ball_y = 0.2;
        g.ball_dx = 0.0;
        g.ball_dy = -0.5;
        g.tick();
        assert!(g.ball_dy > 0.0, "should have reflected off the ceiling");
    }

    #[test]
    fn the_ball_bounces_off_the_paddle() {
        let mut g = Game::new(100, 0, false, 1, COLS, ROWS);
        kill_all_bricks(&mut g);
        g.phase = Phase::Playing;
        g.paddle_col = 32;
        g.ball_x = 36.0;
        g.ball_y = (g.field_rows - 1) as f64 - 0.4;
        g.ball_dx = 0.0;
        g.ball_dy = 0.5;
        assert_eq!(g.tick(), Effect::Redraw);
        assert!(g.ball_dy < 0.0, "should have bounced up off the paddle");
        assert_eq!(g.balls_left, BALLS_PER_GAME);
    }

    #[test]
    fn a_paddle_hit_near_the_edge_sends_the_ball_out_wider_than_a_hit_in_the_middle() {
        let hit_dx = |hit_offset_from_paddle_left: f64| {
            let mut g = Game::new(100, 0, false, 1, COLS, ROWS);
            kill_all_bricks(&mut g);
            g.phase = Phase::Playing;
            g.paddle_col = 30;
            g.ball_x = g.paddle_col as f64 + hit_offset_from_paddle_left;
            g.ball_y = (g.field_rows - 1) as f64 - 0.2;
            g.ball_dx = 0.0;
            g.ball_dy = 0.5;
            g.tick();
            g.ball_dx
        };

        let centre_dx = hit_dx(PADDLE_WIDTH as f64 / 2.0);
        let edge_dx = hit_dx(0.0);

        assert_eq!(
            centre_dx, 0.0,
            "dead centre should send the ball straight up"
        );
        assert!(
            edge_dx.abs() > centre_dx.abs(),
            "an edge hit should be wider than a centre hit: edge={edge_dx} centre={centre_dx}"
        );
        assert!(
            edge_dx < 0.0,
            "a hit on the left edge should send the ball left"
        );
    }

    #[test]
    fn a_paddle_bounce_keeps_the_vertical_speed_steady_however_wide_the_horizontal_gets() {
        let mut g = Game::new(100, 0, false, 1, COLS, ROWS);
        kill_all_bricks(&mut g);
        g.phase = Phase::Playing;
        g.paddle_col = 30;
        // The widest possible kick: a hit right on the paddle's edge.
        g.ball_x = g.paddle_col as f64;
        g.ball_y = (g.field_rows - 1) as f64 - 0.2;
        g.ball_dx = 0.0;
        let incoming_dy = 0.5;
        g.ball_dy = incoming_dy;
        g.tick();
        assert!(
            (g.ball_dy + incoming_dy).abs() < 1e-9,
            "the vertical speed should only flip sign, never shrink: {}",
            g.ball_dy
        );
        assert!(g.ball_dy < 0.0, "should be climbing back toward the bricks");
    }

    #[test]
    fn the_ball_bounces_off_a_brick_and_removes_it() {
        let mut g = Game::new(600, 0, false, 1, COLS, ROWS);
        kill_all_bricks(&mut g);
        g.bricks[0].alive = true;
        let brick_kb = g.bricks[0].kb;
        let before_remaining = g.remaining_kb;
        g.phase = Phase::Playing;
        g.ball_x = 3.0;
        g.ball_y = 0.5;
        g.ball_dx = 0.0;
        g.ball_dy = -0.5;
        g.tick();
        assert!(!g.bricks[0].alive);
        assert_eq!(g.remaining_kb, before_remaining - brick_kb);
        assert!(g.ball_dy > 0.0, "should have bounced down off the brick");
        assert_eq!(g.bricks_broken, 1);
    }

    #[test]
    fn missing_the_paddle_loses_a_ball_and_a_fresh_one_reattaches() {
        let mut g = Game::new(100, 0, false, 1, COLS, ROWS);
        kill_all_bricks(&mut g);
        g.phase = Phase::Playing;
        g.paddle_col = 32;
        g.ball_x = 10.0; // well outside the paddle
        g.ball_y = (g.field_rows - 1) as f64 - 0.4;
        g.ball_dx = 0.0;
        g.ball_dy = 0.5;
        assert_eq!(g.tick(), Effect::Redraw);
        assert_eq!(g.balls_left, BALLS_PER_GAME - 1);
        assert_eq!(g.phase, Phase::Ready);
    }

    #[test]
    fn losing_all_three_balls_ends_the_game_after_the_fail_pause() {
        let mut g = Game::new(100, 0, false, 1, COLS, ROWS);
        kill_all_bricks(&mut g);
        g.paddle_col = 32;

        for expected_balls_left in [2u8, 1, 0] {
            g.phase = Phase::Playing;
            g.ball_x = 10.0;
            g.ball_y = (g.field_rows - 1) as f64 - 0.4;
            g.ball_dx = 0.0;
            g.ball_dy = 0.5;
            g.tick();
            assert_eq!(g.balls_left, expected_balls_left);
        }
        assert_eq!(g.phase, Phase::FailPause { ticks_left: 30 });

        let mut ended_count = 0;
        for _ in 0..FAIL_PAUSE_TICKS {
            if g.tick() == Effect::Ended {
                ended_count += 1;
            }
        }
        assert_eq!(ended_count, 1, "the game ends exactly once");
        assert_eq!(g.phase, Phase::Result { won: false });
    }

    /// The lowest brick row, row four, is exactly `BRICK_ROWS - 1`; column eleven, the last
    /// column, sits wherever `brick_widths` puts it for this field's width.
    fn last_brick_centre_x(field_cols: usize) -> f64 {
        let widths = brick_widths(field_cols);
        let start: usize = widths[..BRICKS_PER_ROW - 1].iter().sum();
        (start + widths[BRICKS_PER_ROW - 1] / 2) as f64
    }

    #[test]
    fn clearing_every_brick_wins_and_ends_exactly_once() {
        let mut g = Game::new(60, 0, false, 1, COLS, ROWS);
        kill_all_bricks(&mut g);
        g.bricks[TOTAL_BRICKS - 1].alive = true;
        g.remaining_kb = g.bricks[TOTAL_BRICKS - 1].kb;
        g.phase = Phase::Playing;
        g.ball_x = last_brick_centre_x(g.field_cols);
        g.ball_y = (BRICK_ROWS - 1) as f64;
        g.ball_dx = 0.0;
        g.ball_dy = 0.0;
        let effect = g.tick();
        assert_eq!(effect, Effect::Ended);
        assert_eq!(g.remaining_kb, 0);
        assert!(g.cleared);
        assert_eq!(g.phase, Phase::Result { won: true });
        // Ended is a one time signal: nothing further claims it again.
        assert_eq!(g.tick(), Effect::Nothing);
    }

    #[test]
    fn speed_increases_by_five_percent_every_ten_bricks() {
        let mut g = Game::new(6000, 0, false, 1, COLS, ROWS);
        g.bricks_broken = 9;
        g.phase = Phase::Playing;
        g.ball_x = 3.0;
        g.ball_y = 0.5;
        g.ball_dx = 0.0;
        g.ball_dy = -0.5;
        g.tick();
        assert_eq!(g.bricks_broken, 10);
        assert!((g.speed - 1.05).abs() < 1e-9, "speed was {}", g.speed);
    }

    #[test]
    fn speed_never_exceeds_one_and_a_half_times_even_after_clearing_every_brick() {
        let mut g = Game::new(6000, 0, false, 1, COLS, ROWS);
        g.phase = Phase::Playing;
        let widths = brick_widths(g.field_cols);
        for row in 0..BRICK_ROWS {
            for col in 0..BRICKS_PER_ROW {
                let start: usize = widths[..col].iter().sum();
                let x = (start + widths[col] / 2) as f64;
                let y = row as f64;
                g.hit_brick(x, y);
            }
        }
        assert_eq!(g.remaining_kb, 0);
        assert!(g.speed <= SPEED_CAP + 1e-9, "speed was {}", g.speed);
        assert!(
            (g.speed - 1.05f64.powi((TOTAL_BRICKS / 10) as i32)).abs() < 1e-9,
            "sixty bricks should not even reach the cap: speed was {}",
            g.speed
        );
    }

    #[test]
    fn the_ball_takes_about_a_second_and_a_half_to_fall_from_the_lowest_brick_row_to_the_bat_at_any_size(
    ) {
        for (cols, rows) in [(80u16, 24u16), (120, 40), (160, 50)] {
            let mut g = Game::new(100, 0, false, 1, cols, rows);
            kill_all_bricks(&mut g);
            g.phase = Phase::Playing;
            g.paddle_col = (g.field_cols - PADDLE_WIDTH) / 2;
            g.ball_x = g.paddle_col as f64 + PADDLE_WIDTH as f64 / 2.0;
            g.ball_y = (BRICK_ROWS - 1) as f64;
            g.ball_dx = 0.0;
            let (base_vertical_step, _) = base_steps(g.field_rows);
            g.ball_dy = base_vertical_step;

            let mut ticks = 0u32;
            loop {
                g.tick();
                ticks += 1;
                assert!(
                    ticks < 1000,
                    "{cols}x{rows}: the ball never reached the bat"
                );
                if g.ball_dy < 0.0 {
                    break;
                }
            }
            let seconds = ticks as f64 / TICKS_PER_SECOND;
            assert!(
                (seconds - 1.5).abs() < 0.15,
                "{cols}x{rows}: expected about 1.5s, took {seconds}s ({ticks} ticks)"
            );
        }
    }

    #[test]
    fn esc_ends_the_game_as_a_loss_and_keeps_what_was_cleared() {
        let mut g = Game::new(100, 0, false, 1, COLS, ROWS);
        // Ended, not Exit: the caller saves on Ended, so giving up halfway still scores what it
        // cleared. Ctrl-C is the way out that records nothing.
        assert_eq!(g.key(Key::Esc), Effect::Ended);
        assert_eq!(g.phase, Phase::Result { won: false });
        assert_eq!(
            g.key(Key::Left),
            Effect::Exit,
            "the result screen dismisses"
        );
    }

    #[test]
    fn ctrl_c_walks_away_without_recording_anything() {
        for phase_setup in [0u8, 1, 2] {
            let mut g = Game::new(600, 0, false, 1, COLS, ROWS);
            if phase_setup >= 1 {
                g.key(Key::Space);
            }
            if phase_setup == 2 {
                g.balls_left = 1;
                g.lose_ball();
                assert!(matches!(g.phase, Phase::FailPause { .. }));
            }
            // Never Ended, in any phase, so the caller never reaches its save.
            assert_eq!(g.key(Key::CtrlC), Effect::Exit, "phase {phase_setup}");
        }
    }

    /// Plays the model with a paddle that simply keeps itself under the ball, and reports the
    /// bricks broken and the balls left. This is the playability check in miniature: a field the
    /// ball cannot reach, or physics that trap it in a band, shows up here as a brick count that
    /// barely moves, which is exactly how the first version of this screen shipped.
    fn follow_the_ball(cols: u16, rows: u16, ticks: usize) -> (u32, u8) {
        let mut g = Game::new(18_874_368, 0, false, 1, cols, rows);
        g.key(Key::Space);
        for _ in 0..ticks {
            let centre = g.paddle_col as f64 + PADDLE_WIDTH as f64 / 2.0;
            if g.ball_x < centre - 1.0 {
                g.key(Key::Left);
            } else if g.ball_x > centre + 1.0 {
                g.key(Key::Right);
            }
            if g.tick() == Effect::Ended {
                break;
            }
            if g.phase == Phase::Ready {
                g.key(Key::Space);
            }
        }
        (g.bricks_broken, g.balls_left)
    }

    #[test]
    fn a_minute_of_following_the_ball_clears_ten_bricks_and_loses_none_at_any_size() {
        // The bar the maintainer set after the first version proved unplayable: a bat that only
        // tracks the ball has to manage ten bricks on one ball, in a minute, however big the
        // terminal is: the whole point of scaling the ball's speed to the field. Measured here at
        // 19 a minute at 80x24, 22 at 120x40, 23 at 160x50.
        for (cols, rows) in [(80u16, 24u16), (120, 40), (160, 50)] {
            let (broken, balls) = follow_the_ball(cols, rows, 30 * 60);
            assert!(
                broken >= 10,
                "{cols}x{rows}: only {broken} bricks in a minute"
            );
            assert_eq!(
                balls, BALLS_PER_GAME,
                "{cols}x{rows}: a ball was lost to a bat that was there"
            );
        }
    }

    #[test]
    fn either_way_in_starts_the_ball_attached_and_ready() {
        let g = Game::new(37748736, 0, false, 42, COLS, ROWS);
        assert_eq!(g.phase, Phase::Ready);
        assert_eq!(g.balls_left, BALLS_PER_GAME);
        assert_eq!(g.remaining_kb, g.mem_kb);
    }

    #[test]
    fn space_launches_only_while_the_ball_is_ready() {
        let mut g = Game::new(100, 0, false, 1, COLS, ROWS);
        assert_eq!(g.key(Key::Space), Effect::Redraw);
        assert_eq!(g.phase, Phase::Playing);
        assert_eq!(g.key(Key::Space), Effect::Nothing, "already flying");
    }

    #[test]
    fn moving_the_paddle_keeps_the_attached_ball_above_its_centre() {
        let mut g = Game::new(100, 0, false, 1, COLS, ROWS);
        g.key(Key::Right);
        assert_eq!(g.ball_x, (g.paddle_col + PADDLE_WIDTH / 2) as f64);
    }

    #[test]
    fn a_new_record_is_whatever_beats_the_best_that_stood_at_the_start() {
        let mut g = Game::new(600, 100, false, 1, COLS, ROWS);
        assert!(!g.is_new_record());
        g.remaining_kb = g.mem_kb - 200;
        assert!(g.is_new_record());
    }
}
