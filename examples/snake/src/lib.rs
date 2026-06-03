//! Grid-based Snake, built on Octiron. Steer with arrows or WASD (no instant
//! 180° reversals). Eat the apple to grow and score; hit a wall or your own
//! body and it's GAME OVER. Press Space to restart. This example drives
//! everything from `Game::draw` with the immediate-mode `Painter`, leaving the
//! ECS `World` empty—the engine is happy painting on its own.

use octiron::{run, Assets, Frame, Game, Key, Painter, Rng, Texture, Vec2, World};
use wasm_bindgen::prelude::*;

const FIELD: Vec2 = Vec2::new(800.0, 600.0);
const CELL: f32 = 40.0;
const COLS: i32 = 20; // 20 * 40 = 800
const ROWS: i32 = 15; // 15 * 40 = 600
const STEP_TIME: f32 = 0.11; // seconds per grid step
const APPLE_SIZE: f32 = 26.0;

/// A grid coordinate (column, row).
#[derive(Clone, Copy, PartialEq, Eq)]
struct Cell {
    x: i32,
    y: i32,
}

impl Cell {
    fn new(x: i32, y: i32) -> Self {
        Cell { x, y }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    fn delta(self) -> Cell {
        match self {
            Dir::Up => Cell::new(0, -1),
            Dir::Down => Cell::new(0, 1),
            Dir::Left => Cell::new(-1, 0),
            Dir::Right => Cell::new(1, 0),
        }
    }
    /// True if `other` is the direct reverse of `self` (a forbidden 180° turn).
    fn is_opposite(self, other: Dir) -> bool {
        matches!(
            (self, other),
            (Dir::Up, Dir::Down)
                | (Dir::Down, Dir::Up)
                | (Dir::Left, Dir::Right)
                | (Dir::Right, Dir::Left)
        )
    }
}

struct Snake {
    /// Body cells; `body[0]` is the head, the tail is last.
    body: Vec<Cell>,
    /// Current travel direction (applied each step).
    dir: Dir,
    /// Buffered direction from input, committed at the next step.
    next_dir: Dir,
    apple: Cell,
    score: u32,
    timer: f32,
    game_over: bool,
    rng: Rng,
}

impl Default for Snake {
    fn default() -> Self {
        let mut s = Snake {
            body: Vec::new(),
            dir: Dir::Right,
            next_dir: Dir::Right,
            apple: Cell::new(0, 0),
            score: 0,
            timer: 0.0,
            game_over: false,
            rng: Rng::new(0x9e37_79b9),
        };
        s.reset();
        s
    }
}

impl Snake {
    fn reset(&mut self) {
        let mid = ROWS / 2;
        // Three-segment snake near the left, heading right.
        self.body = vec![Cell::new(5, mid), Cell::new(4, mid), Cell::new(3, mid)];
        self.dir = Dir::Right;
        self.next_dir = Dir::Right;
        self.score = 0;
        self.timer = 0.0;
        self.game_over = false;
        self.place_apple();
    }

    /// Picks a free cell for the apple (one not occupied by the body).
    fn place_apple(&mut self) {
        // The board has COLS*ROWS cells and the body is short, so rejection
        // sampling converges quickly.
        loop {
            let candidate = Cell::new(self.rng.below(COLS), self.rng.below(ROWS));
            if !self.body.contains(&candidate) {
                self.apple = candidate;
                return;
            }
        }
    }

    fn step(&mut self) {
        // Commit the buffered direction (already vetted against reversals).
        self.dir = self.next_dir;
        let d = self.dir.delta();
        let head = self.body[0];
        let new_head = Cell::new(head.x + d.x, head.y + d.y);

        // Wall collision.
        if new_head.x < 0 || new_head.x >= COLS || new_head.y < 0 || new_head.y >= ROWS {
            self.game_over = true;
            return;
        }

        let eating = new_head == self.apple;

        // Self collision. When not eating, the tail moves out of the way this
        // step, so its current cell is free to enter.
        let occupied_len = if eating {
            self.body.len()
        } else {
            self.body.len() - 1
        };
        if self.body[..occupied_len].contains(&new_head) {
            self.game_over = true;
            return;
        }

        self.body.insert(0, new_head);
        if eating {
            self.score += 1;
            self.place_apple();
        } else {
            self.body.pop();
        }
    }

    /// Reads input and buffers a new direction (ignoring instant reversals).
    fn read_input(&mut self, frame: &Frame) {
        let requested =
            if frame.input.is_key_down(Key::ArrowUp) || frame.input.is_key_down(Key::KeyW) {
                Some(Dir::Up)
            } else if frame.input.is_key_down(Key::ArrowDown) || frame.input.is_key_down(Key::KeyS) {
                Some(Dir::Down)
            } else if frame.input.is_key_down(Key::ArrowLeft) || frame.input.is_key_down(Key::KeyA) {
                Some(Dir::Left)
            } else if frame.input.is_key_down(Key::ArrowRight) || frame.input.is_key_down(Key::KeyD)
            {
                Some(Dir::Right)
            } else {
                None
            };
        if let Some(d) = requested {
            // Compare against the committed direction so a quick double-tap
            // can't sneak a reversal in before the next step.
            if !self.dir.is_opposite(d) {
                self.next_dir = d;
            }
        }
    }
}

/// Converts a grid cell's top-left to pixel coordinates.
fn cell_to_px(c: Cell) -> Vec2 {
    Vec2::new(c.x as f32 * CELL, c.y as f32 * CELL)
}

struct SnakeGame {
    snake: Snake,
    apple_tex: Texture,
}

impl Default for SnakeGame {
    fn default() -> Self {
        SnakeGame {
            snake: Snake::default(),
            apple_tex: Texture::WHITE,
        }
    }
}

impl Game for SnakeGame {
    fn start(&mut self, _world: &mut World, assets: &mut Assets) {
        self.apple_tex = assets.load_png(include_bytes!("../assets/apple.png"));
    }

    fn update(&mut self, _world: &mut World, frame: &Frame) {
        if self.snake.game_over {
            if frame.input.is_key_down(Key::Space) {
                self.snake.reset();
            }
            return;
        }

        self.snake.read_input(frame);

        self.snake.timer += frame.dt;
        while self.snake.timer >= STEP_TIME && !self.snake.game_over {
            self.snake.timer -= STEP_TIME;
            self.snake.step();
        }
    }

    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        // Subtle checkerboard so the grid is legible.
        for y in 0..ROWS {
            for x in 0..COLS {
                let dark = (x + y) % 2 == 0;
                let shade = if dark {
                    [0.10, 0.12, 0.16, 1.0]
                } else {
                    [0.12, 0.15, 0.20, 1.0]
                };
                painter.rect(x as f32 * CELL, y as f32 * CELL, CELL, CELL, shade);
            }
        }

        // Apple, centered in its cell.
        let apple_px = cell_to_px(self.snake.apple);
        let inset = (CELL - APPLE_SIZE) * 0.5;
        painter.sprite(
            self.apple_tex,
            apple_px.x + inset,
            apple_px.y + inset,
            APPLE_SIZE,
            APPLE_SIZE,
            [1.0, 1.0, 1.0, 1.0],
        );

        // Snake body: head bright, body darker, each slightly inset from the
        // cell so segments read as distinct.
        let pad = 3.0;
        for (i, cell) in self.snake.body.iter().enumerate() {
            let px = cell_to_px(*cell);
            let color = if i == 0 {
                [0.45, 0.95, 0.45, 1.0]
            } else {
                [0.20, 0.65, 0.28, 1.0]
            };
            painter.rect(
                px.x + pad,
                px.y + pad,
                CELL - pad * 2.0,
                CELL - pad * 2.0,
                color,
            );
        }

        // HUD: score top-left.
        painter.text(
            14.0,
            12.0,
            0.7,
            [0.85, 1.0, 0.85, 1.0],
            &format!("SCORE {}", self.snake.score),
        );

        // Game over overlay.
        if self.snake.game_over {
            // Dim the field.
            painter.rect(0.0, 0.0, FIELD.x, FIELD.y, [0.0, 0.0, 0.0, 0.55]);
            painter.text_centered(
                FIELD.x * 0.5,
                FIELD.y * 0.5 - 40.0,
                1.3,
                [1.0, 0.55, 0.5, 1.0],
                "GAME OVER",
            );
            painter.text_centered(
                FIELD.x * 0.5,
                FIELD.y * 0.5 + 16.0,
                0.6,
                [0.9, 0.9, 0.95, 1.0],
                "PRESS SPACE TO RESTART",
            );
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    run(SnakeGame::default());
}
