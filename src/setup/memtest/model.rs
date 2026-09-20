//! The Memory Test easter egg: a small Breakout, pure. No terminal, no clock, no randomness
//! beyond the seed the caller supplies once, at construction. Everything the game does in
//! response to a key or a tick is decided here, so it can all be tested without a tty.

/// A key the game understands. Anything else is ignored by the caller before it ever reaches
/// here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
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
    Redraw,
    /// The game has just reached its one true ending: every ball lost, or every brick cleared.
    /// This is the single moment the result should be saved. Giving up never produces this.
    Ended,
    /// Leave the game and return to setup. Nothing is saved.
    Exit,
}

/// The playfield: 72 columns by 10 rows, same units `view` draws in.
pub const FIELD_COLS: usize = 72;
pub const FIELD_ROWS: usize = 10;
pub const BRICK_ROWS: usize = 5;
pub const BRICKS_PER_ROW: usize = 12;
pub const BRICK_WIDTH: usize = 6;
pub const TOTAL_BRICKS: usize = BRICK_ROWS * BRICKS_PER_ROW;
pub const PADDLE_WIDTH: usize = 8;

/// Brick row colours, top to bottom: red, yellow, green, cyan, blue. The rainbow order used
/// elsewhere, as plain ANSI SGR foreground codes.
pub const BRICK_COLOURS: [u8; BRICK_ROWS] = [31, 33, 32, 36, 34];

const PADDLE_STEP: i32 = 3;
const BALLS_PER_GAME: u8 = 3;
/// The ball's base speed: half a cell per tick at 30 frames per second, before the ten percent
/// per ten bricks speed up.
const BASE_STEP: f64 = 0.5;
/// One second at 30 frames per second: how long the header holds `FAIL` before the result screen.
const FAIL_PAUSE_TICKS: u8 = 30;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Brick {
    pub alive: bool,
    pub kb: u64,
}

/// Which part of a game is showing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    /// The ball rests on the paddle, waiting for Space.
    Ready,
    Playing,
    /// The header holds `FAIL` for a beat before the result screen appears.
    FailPause {
        ticks_left: u8,
    },
    /// The result screen: waiting for any key to leave.
    Result {
        won: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub bricks: Vec<Brick>,
    pub paddle_col: usize,
    pub ball_x: f64,
    pub ball_y: f64,
    pub ball_dx: f64,
    pub ball_dy: f64,
    pub balls_left: u8,
    pub mem_kb: u64,
    pub remaining_kb: u64,
    pub bricks_broken: u32,
    pub speed: f64,
    pub phase: Phase,
    /// The best K ever cleared before this game started. Never changed by play; the result
    /// screen compares this game's score against it to decide whether to say `New record.`
    pub best_kb: u64,
    /// Whether any game has ever been fully cleared, including this one from the moment it is.
    pub cleared: bool,
    seed: u64,
}

impl Game {
    /// A fresh game over `mem_kb` of memory. `best_kb` and `cleared_before` come from
    /// `state.json`. `seed` decides the launch angle deterministically; the model never generates
    /// randomness of its own.
    pub fn new(mem_kb: u64, best_kb: u64, cleared_before: bool, seed: u64) -> Game {
        let paddle_col = (FIELD_COLS - PADDLE_WIDTH) / 2;
        let mut game = Game {
            bricks: build_bricks(mem_kb),
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
        let max_col = (FIELD_COLS - PADDLE_WIDTH) as i32;
        let step = PADDLE_STEP * direction;
        let new_col = (self.paddle_col as i32 + step).clamp(0, max_col);
        self.paddle_col = new_col as usize;
        if self.phase == Phase::Ready {
            self.place_ball_on_paddle();
        }
    }

    fn place_ball_on_paddle(&mut self) {
        self.ball_x = (self.paddle_col + PADDLE_WIDTH / 2) as f64;
        self.ball_y = (FIELD_ROWS - 1 - 2) as f64;
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
        self.ball_dx = BASE_STEP * self.speed * direction;
        self.ball_dy = -BASE_STEP * self.speed;
    }

    /// One tick of ball physics: walls, the paddle, and bricks, in that order. Returns the effect
    /// for the caller, same as `key`.
    fn advance_ball(&mut self) -> Effect {
        let mut next_x = self.ball_x + self.ball_dx;
        if next_x < 0.0 || next_x > (FIELD_COLS - 1) as f64 {
            self.ball_dx = -self.ball_dx;
            next_x = self.ball_x + self.ball_dx;
        }

        let mut next_y = self.ball_y + self.ball_dy;
        if next_y < 0.0 {
            self.ball_dy = -self.ball_dy;
            next_y = self.ball_y + self.ball_dy;
        }

        let paddle_row = (FIELD_ROWS - 1) as f64;
        if next_y >= paddle_row {
            let paddle_left = self.paddle_col as f64;
            let paddle_right = paddle_left + PADDLE_WIDTH as f64;
            if next_x >= paddle_left && next_x < paddle_right {
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
        let col = (x.round().clamp(0.0, (FIELD_COLS - 1) as f64)) as usize / BRICK_WIDTH;
        let idx = row as usize * BRICKS_PER_ROW + col;
        let brick = self.bricks.get_mut(idx)?;
        if !brick.alive {
            return None;
        }

        brick.alive = false;
        self.remaining_kb -= brick.kb;
        self.bricks_broken += 1;
        if self.bricks_broken % 10 == 0 {
            self.speed *= 1.1;
            self.ball_dx *= 1.1;
            self.ball_dy *= 1.1;
        }
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
    fn the_ball_bounces_off_the_left_wall() {
        let mut g = Game::new(100, 0, false, 1);
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
        let mut g = Game::new(100, 0, false, 1);
        kill_all_bricks(&mut g);
        g.phase = Phase::Playing;
        g.ball_x = (FIELD_COLS - 1) as f64 - 0.2;
        g.ball_y = 5.0;
        g.ball_dx = 0.5;
        g.ball_dy = 0.0;
        g.tick();
        assert!(g.ball_dx < 0.0, "should have reflected off the right wall");
    }

    #[test]
    fn the_ball_bounces_off_the_ceiling() {
        let mut g = Game::new(100, 0, false, 1);
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
        let mut g = Game::new(100, 0, false, 1);
        kill_all_bricks(&mut g);
        g.phase = Phase::Playing;
        g.paddle_col = 32;
        g.ball_x = 36.0;
        g.ball_y = 8.6;
        g.ball_dx = 0.0;
        g.ball_dy = 0.5;
        assert_eq!(g.tick(), Effect::Redraw);
        assert!(g.ball_dy < 0.0, "should have bounced up off the paddle");
        assert_eq!(g.balls_left, BALLS_PER_GAME);
    }

    #[test]
    fn the_ball_bounces_off_a_brick_and_removes_it() {
        let mut g = Game::new(600, 0, false, 1);
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
        let mut g = Game::new(100, 0, false, 1);
        kill_all_bricks(&mut g);
        g.phase = Phase::Playing;
        g.paddle_col = 32;
        g.ball_x = 10.0; // well outside the paddle
        g.ball_y = 8.6;
        g.ball_dx = 0.0;
        g.ball_dy = 0.5;
        assert_eq!(g.tick(), Effect::Redraw);
        assert_eq!(g.balls_left, BALLS_PER_GAME - 1);
        assert_eq!(g.phase, Phase::Ready);
    }

    #[test]
    fn losing_all_three_balls_ends_the_game_after_the_fail_pause() {
        let mut g = Game::new(100, 0, false, 1);
        kill_all_bricks(&mut g);
        g.paddle_col = 32;

        for expected_balls_left in [2u8, 1, 0] {
            g.phase = Phase::Playing;
            g.ball_x = 10.0;
            g.ball_y = 8.6;
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

    #[test]
    fn clearing_every_brick_wins_and_ends_exactly_once() {
        let mut g = Game::new(60, 0, false, 1);
        kill_all_bricks(&mut g);
        g.bricks[TOTAL_BRICKS - 1].alive = true;
        g.remaining_kb = g.bricks[TOTAL_BRICKS - 1].kb;
        g.phase = Phase::Playing;
        // Brick 59 is row 4, column 11: x in 66..72, y == 4.
        g.ball_x = 70.0;
        g.ball_y = 4.0;
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
    fn speed_increases_by_a_tenth_every_ten_bricks() {
        let mut g = Game::new(6000, 0, false, 1);
        g.bricks_broken = 9;
        g.phase = Phase::Playing;
        g.ball_x = 3.0;
        g.ball_y = 0.5;
        g.ball_dx = 0.0;
        g.ball_dy = -0.5;
        g.tick();
        assert_eq!(g.bricks_broken, 10);
        assert!((g.speed - 1.1).abs() < 1e-9, "speed was {}", g.speed);
    }

    #[test]
    fn esc_ends_the_game_as_a_loss_and_keeps_what_was_cleared() {
        let mut g = Game::new(100, 0, false, 1);
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
            let mut g = Game::new(600, 0, false, 1);
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

    #[test]
    fn either_way_in_starts_the_ball_attached_and_ready() {
        let g = Game::new(37748736, 0, false, 42);
        assert_eq!(g.phase, Phase::Ready);
        assert_eq!(g.balls_left, BALLS_PER_GAME);
        assert_eq!(g.remaining_kb, g.mem_kb);
    }

    #[test]
    fn space_launches_only_while_the_ball_is_ready() {
        let mut g = Game::new(100, 0, false, 1);
        assert_eq!(g.key(Key::Space), Effect::Redraw);
        assert_eq!(g.phase, Phase::Playing);
        assert_eq!(g.key(Key::Space), Effect::Nothing, "already flying");
    }

    #[test]
    fn moving_the_paddle_keeps_the_attached_ball_above_its_centre() {
        let mut g = Game::new(100, 0, false, 1);
        g.key(Key::Right);
        assert_eq!(g.ball_x, (g.paddle_col + PADDLE_WIDTH / 2) as f64);
    }

    #[test]
    fn a_new_record_is_whatever_beats_the_best_that_stood_at_the_start() {
        let mut g = Game::new(600, 100, false, 1);
        assert!(!g.is_new_record());
        g.remaining_kb = g.mem_kb - 200;
        assert!(g.is_new_record());
    }
}
