//! Tetris — a heavily stateful example: a board model, seven tetrominoes with
//! four rotation states each (plus wall kicks), DAS auto-shift, a ghost piece,
//! line clears, level-based gravity, and a `SceneStack` (title / play / over).
//! Pure `Painter` rendering, no textures.
//!
//! ←/→ move · ↑/X rotate · ↓ soft drop · Space hard drop.

use octiron::{
    run, Assets, Color, Frame, Game, Key, Painter, Rng, Scene, SceneStack, Transition, Vec2, World,
};
use wasm_bindgen::prelude::*;

const SCREEN: Vec2 = Vec2::new(800.0, 600.0);
const COLS: i32 = 10;
const ROWS: i32 = 20;
const CELL: f32 = 26.0;
const BOARD_X: f32 = 250.0;
const BOARD_Y: f32 = 40.0;
const DAS_DELAY: f32 = 0.16;
const DAS_REPEAT: f32 = 0.05;
const SOFT_DROP: f32 = 0.04;

const COLORS: [Color; 7] = [
    [0.32, 0.80, 0.93, 1.0], // I cyan
    [0.95, 0.82, 0.28, 1.0], // O yellow
    [0.72, 0.42, 0.88, 1.0], // T purple
    [0.42, 0.82, 0.45, 1.0], // S green
    [0.92, 0.38, 0.38, 1.0], // Z red
    [0.36, 0.56, 0.93, 1.0], // J blue
    [0.96, 0.60, 0.26, 1.0], // L orange
];

// 7 pieces, 4 rotations, 4 cells each (col,row) inside a 4x4 box.
const SHAPES: [[[(i32, i32); 4]; 4]; 7] = [
    // I
    [
        [(0, 1), (1, 1), (2, 1), (3, 1)],
        [(2, 0), (2, 1), (2, 2), (2, 3)],
        [(0, 2), (1, 2), (2, 2), (3, 2)],
        [(1, 0), (1, 1), (1, 2), (1, 3)],
    ],
    // O
    [
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
    ],
    // T
    [
        [(1, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (1, 1), (2, 1), (1, 2)],
        [(0, 1), (1, 1), (2, 1), (1, 2)],
        [(1, 0), (0, 1), (1, 1), (1, 2)],
    ],
    // S
    [
        [(1, 0), (2, 0), (0, 1), (1, 1)],
        [(1, 0), (1, 1), (2, 1), (2, 2)],
        [(1, 1), (2, 1), (0, 2), (1, 2)],
        [(0, 0), (0, 1), (1, 1), (1, 2)],
    ],
    // Z
    [
        [(0, 0), (1, 0), (1, 1), (2, 1)],
        [(2, 0), (1, 1), (2, 1), (1, 2)],
        [(0, 1), (1, 1), (1, 2), (2, 2)],
        [(1, 0), (0, 1), (1, 1), (0, 2)],
    ],
    // J
    [
        [(0, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (1, 2)],
        [(0, 1), (1, 1), (2, 1), (2, 2)],
        [(1, 0), (1, 1), (0, 2), (1, 2)],
    ],
    // L
    [
        [(2, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (1, 1), (1, 2), (2, 2)],
        [(0, 1), (1, 1), (2, 1), (0, 2)],
        [(0, 0), (1, 0), (1, 1), (1, 2)],
    ],
];

fn draw_block(painter: &mut Painter, px: f32, py: f32, color: Color) {
    let dark = [color[0] * 0.6, color[1] * 0.6, color[2] * 0.6, color[3]];
    let light = [
        (color[0] * 1.3).min(1.0),
        (color[1] * 1.3).min(1.0),
        (color[2] * 1.3).min(1.0),
        color[3],
    ];
    painter.rect(px, py, CELL, CELL, dark);
    painter.rect(px + 2.0, py + 2.0, CELL - 4.0, CELL - 4.0, color);
    painter.rect(px + 2.0, py + 2.0, CELL - 4.0, (CELL - 4.0) * 0.34, light);
}

// ---------------------------------------------------------------------------

struct Title;

impl Scene for Title {
    fn enter(&mut self, world: &mut World) {
        world.clear();
    }
    fn update(&mut self, _world: &mut World, frame: &Frame) -> Transition {
        if frame.input.is_key_pressed(Key::Space) || frame.input.is_key_pressed(Key::Enter) {
            Transition::Replace(Box::new(Play::new()))
        } else {
            Transition::None
        }
    }
    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        painter.text_centered(SCREEN.x * 0.5, 180.0, 1.7, [1.0, 1.0, 1.0, 1.0], "OCTIRON");
        painter.text_centered(SCREEN.x * 0.5, 260.0, 1.0, [0.5, 0.85, 1.0, 1.0], "TETRIS");
        painter.text_centered(
            SCREEN.x * 0.5,
            370.0,
            0.5,
            [0.9, 0.92, 1.0, 1.0],
            "MOVE ARROWS   ROTATE UP/X   DROP SPACE",
        );
        painter.text_centered(SCREEN.x * 0.5, 430.0, 0.7, [0.7, 1.0, 0.7, 1.0], "PRESS SPACE TO START");
    }
}

struct GameOver {
    score: u32,
    lines: u32,
}

impl Scene for GameOver {
    fn enter(&mut self, world: &mut World) {
        world.clear();
    }
    fn update(&mut self, _world: &mut World, frame: &Frame) -> Transition {
        if frame.input.is_key_pressed(Key::Space) || frame.input.is_key_pressed(Key::Enter) {
            Transition::Replace(Box::new(Title))
        } else {
            Transition::None
        }
    }
    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        painter.text_centered(SCREEN.x * 0.5, 210.0, 1.5, [1.0, 0.55, 0.55, 1.0], "GAME OVER");
        painter.text_centered(
            SCREEN.x * 0.5,
            300.0,
            0.7,
            [1.0, 0.9, 0.4, 1.0],
            &format!("SCORE  {}", self.score),
        );
        painter.text_centered(
            SCREEN.x * 0.5,
            340.0,
            0.55,
            [0.8, 0.9, 1.0, 1.0],
            &format!("LINES  {}", self.lines),
        );
        painter.text_centered(SCREEN.x * 0.5, 420.0, 0.55, [0.85, 0.85, 0.95, 1.0], "PRESS SPACE");
    }
}

struct Play {
    board: [i8; (COLS * ROWS) as usize],
    cur: usize,
    rot: usize,
    px: i32,
    py: i32,
    next: usize,
    rng: Rng,
    fall: f32,
    das: f32,
    score: u32,
    lines: u32,
    level: u32,
}

impl Play {
    fn new() -> Play {
        let mut rng = Rng::new(0x51ED_2701);
        let cur = rng.below(7) as usize;
        let next = rng.below(7) as usize;
        Play {
            board: [-1; (COLS * ROWS) as usize],
            cur,
            rot: 0,
            px: 3,
            py: 0,
            next,
            rng,
            fall: 0.0,
            das: DAS_DELAY,
            score: 0,
            lines: 0,
            level: 1,
        }
    }

    fn fits(&self, cur: usize, rot: usize, px: i32, py: i32) -> bool {
        for (dx, dy) in SHAPES[cur][rot] {
            let c = px + dx;
            let r = py + dy;
            if c < 0 || c >= COLS || r < 0 || r >= ROWS {
                return false;
            }
            if self.board[(r * COLS + c) as usize] >= 0 {
                return false;
            }
        }
        true
    }

    fn gravity(&self) -> f32 {
        (0.8 - (self.level - 1) as f32 * 0.06).max(0.05)
    }

    fn drop_row(&self) -> i32 {
        let mut y = self.py;
        while self.fits(self.cur, self.rot, self.px, y + 1) {
            y += 1;
        }
        y
    }

    /// Locks the current piece, clears full rows, spawns the next. Returns
    /// `false` if the next piece can't spawn (game over).
    fn lock_and_spawn(&mut self) -> bool {
        for (dx, dy) in SHAPES[self.cur][self.rot] {
            let c = self.px + dx;
            let r = self.py + dy;
            if r >= 0 && r < ROWS && c >= 0 && c < COLS {
                self.board[(r * COLS + c) as usize] = self.cur as i8;
            }
        }

        let mut cleared = 0u32;
        let mut write = ROWS - 1;
        for read in (0..ROWS).rev() {
            let full = (0..COLS).all(|c| self.board[(read * COLS + c) as usize] >= 0);
            if full {
                cleared += 1;
            } else {
                if write != read {
                    for c in 0..COLS {
                        self.board[(write * COLS + c) as usize] =
                            self.board[(read * COLS + c) as usize];
                    }
                }
                write -= 1;
            }
        }
        for r in 0..=write {
            for c in 0..COLS {
                self.board[(r * COLS + c) as usize] = -1;
            }
        }

        if cleared > 0 {
            let base = [0, 100, 300, 500, 800][cleared as usize];
            self.score += base * self.level;
            self.lines += cleared;
            self.level = self.lines / 10 + 1;
        }

        self.cur = self.next;
        self.next = self.rng.below(7) as usize;
        self.rot = 0;
        self.px = 3;
        self.py = 0;
        self.fall = 0.0;
        self.fits(self.cur, self.rot, self.px, self.py)
    }
}

impl Scene for Play {
    fn enter(&mut self, world: &mut World) {
        world.clear();
    }

    fn update(&mut self, _world: &mut World, frame: &Frame) -> Transition {
        let dt = frame.dt;
        let input = frame.input;

        // Horizontal move with auto-shift (DAS).
        let left_d = input.is_key_down(Key::ArrowLeft) || input.is_key_down(Key::KeyA);
        let right_d = input.is_key_down(Key::ArrowRight) || input.is_key_down(Key::KeyD);
        let left_p = input.is_key_pressed(Key::ArrowLeft) || input.is_key_pressed(Key::KeyA);
        let right_p = input.is_key_pressed(Key::ArrowRight) || input.is_key_pressed(Key::KeyD);
        let mut hmove = 0;
        if left_p {
            hmove = -1;
            self.das = DAS_DELAY;
        } else if right_p {
            hmove = 1;
            self.das = DAS_DELAY;
        } else if left_d || right_d {
            self.das -= dt;
            if self.das <= 0.0 {
                hmove = if left_d { -1 } else { 1 };
                self.das = DAS_REPEAT;
            }
        }
        if hmove != 0 && self.fits(self.cur, self.rot, self.px + hmove, self.py) {
            self.px += hmove;
        }

        // Rotate with simple wall kicks.
        if input.is_key_pressed(Key::ArrowUp)
            || input.is_key_pressed(Key::KeyX)
            || input.is_key_pressed(Key::KeyW)
        {
            let nrot = (self.rot + 1) % 4;
            for kick in [0, -1, 1, -2, 2] {
                if self.fits(self.cur, nrot, self.px + kick, self.py) {
                    self.rot = nrot;
                    self.px += kick;
                    break;
                }
            }
        }

        // Hard drop.
        if input.is_key_pressed(Key::Space) {
            let y = self.drop_row();
            self.score += (y - self.py).max(0) as u32 * 2;
            self.py = y;
            if !self.lock_and_spawn() {
                return Transition::Replace(Box::new(GameOver {
                    score: self.score,
                    lines: self.lines,
                }));
            }
            return Transition::None;
        }

        // Gravity (soft drop accelerates).
        let soft = input.is_key_down(Key::ArrowDown) || input.is_key_down(Key::KeyS);
        let interval = if soft { SOFT_DROP } else { self.gravity() };
        self.fall += dt;
        while self.fall >= interval {
            self.fall -= interval;
            if self.fits(self.cur, self.rot, self.px, self.py + 1) {
                self.py += 1;
                if soft {
                    self.score += 1;
                }
            } else if !self.lock_and_spawn() {
                return Transition::Replace(Box::new(GameOver {
                    score: self.score,
                    lines: self.lines,
                }));
            }
        }

        Transition::None
    }

    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        let bw = COLS as f32 * CELL;
        let bh = ROWS as f32 * CELL;
        // Playfield frame + background.
        painter.rect(BOARD_X - 6.0, BOARD_Y - 6.0, bw + 12.0, bh + 12.0, [0.10, 0.12, 0.17, 1.0]);
        painter.rect(BOARD_X, BOARD_Y, bw, bh, [0.04, 0.05, 0.08, 1.0]);

        // Locked blocks.
        for r in 0..ROWS {
            for c in 0..COLS {
                let v = self.board[(r * COLS + c) as usize];
                if v >= 0 {
                    draw_block(
                        painter,
                        BOARD_X + c as f32 * CELL,
                        BOARD_Y + r as f32 * CELL,
                        COLORS[v as usize],
                    );
                }
            }
        }

        // Ghost piece.
        let gy = self.drop_row();
        for (dx, dy) in SHAPES[self.cur][self.rot] {
            let c = self.px + dx;
            let r = gy + dy;
            if r >= 0 {
                let col = COLORS[self.cur];
                painter.rect(
                    BOARD_X + c as f32 * CELL + 2.0,
                    BOARD_Y + r as f32 * CELL + 2.0,
                    CELL - 4.0,
                    CELL - 4.0,
                    [col[0], col[1], col[2], 0.18],
                );
            }
        }

        // Active piece.
        for (dx, dy) in SHAPES[self.cur][self.rot] {
            let c = self.px + dx;
            let r = self.py + dy;
            if r >= 0 {
                draw_block(
                    painter,
                    BOARD_X + c as f32 * CELL,
                    BOARD_Y + r as f32 * CELL,
                    COLORS[self.cur],
                );
            }
        }

        // Side panel: NEXT + stats.
        let panel_x = BOARD_X + bw + 36.0;
        painter.text(panel_x, BOARD_Y, 0.6, [0.8, 0.85, 0.95, 1.0], "NEXT");
        let nx = panel_x + 6.0;
        let ny = BOARD_Y + 40.0;
        for (dx, dy) in SHAPES[self.next][0] {
            draw_block(
                painter,
                nx + dx as f32 * CELL,
                ny + dy as f32 * CELL,
                COLORS[self.next],
            );
        }

        let sy = BOARD_Y + 200.0;
        painter.text(panel_x, sy, 0.55, [0.8, 0.85, 0.95, 1.0], "SCORE");
        painter.text(panel_x, sy + 28.0, 0.6, [1.0, 0.9, 0.4, 1.0], &format!("{}", self.score));
        painter.text(panel_x, sy + 80.0, 0.55, [0.8, 0.85, 0.95, 1.0], "LINES");
        painter.text(panel_x, sy + 108.0, 0.6, [0.7, 1.0, 0.8, 1.0], &format!("{}", self.lines));
        painter.text(panel_x, sy + 160.0, 0.55, [0.8, 0.85, 0.95, 1.0], "LEVEL");
        painter.text(panel_x, sy + 188.0, 0.6, [0.7, 0.85, 1.0, 1.0], &format!("{}", self.level));
    }
}

// ---------------------------------------------------------------------------

#[derive(Default)]
struct Tetris {
    stack: Option<SceneStack>,
}

impl Game for Tetris {
    fn start(&mut self, _world: &mut World, _assets: &mut Assets) {
        self.stack = Some(SceneStack::new(Box::new(Title)));
    }
    fn update(&mut self, world: &mut World, frame: &Frame) {
        if let Some(s) = &mut self.stack {
            s.update(world, frame);
        }
    }
    fn draw(&mut self, world: &World, painter: &mut Painter) {
        if let Some(s) = &mut self.stack {
            s.draw(world, painter);
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    run(Tetris::default());
}
