//! A scrolling platformer — the most architecturally involved example. It
//! exercises Octiron's `SceneStack` (title → levels → win / game-over) and the
//! `Game::camera()` world offset, plus tile-grid collision physics.
//!
//! Levels are ASCII maps: `#` solid, `P` player, `s` slime, `o` coin, `G` goal.
//! Move with ←/→ or A/D, jump with Space / ↑ / W.

use octiron::{
    run_with, Assets, Config, Entity, Frame, Game, Key, Painter, Rect, Scene, SceneStack, Sprite,
    Texture, Transition, Transform, Vec2, World,
};
use wasm_bindgen::prelude::*;

const TILE: f32 = 40.0;
const SCREEN: Vec2 = Vec2::new(800.0, 600.0);
const GRAVITY: f32 = 2000.0;
const MAX_FALL: f32 = 1200.0;
const MOVE_SPEED: f32 = 240.0;
const JUMP_SPEED: f32 = 780.0;
const PLAYER_SIZE: Vec2 = Vec2::new(28.0, 38.0);
const ENEMY_SIZE: Vec2 = Vec2::new(36.0, 26.0);
const ENEMY_SPEED: f32 = 70.0;
const COIN_SIZE: f32 = 26.0;
const FLAG_SIZE: Vec2 = Vec2::new(40.0, 56.0);

// 34-wide, full ground: a gentle first level (one platform, one slime far off).
const L1: &[&str] = &[
    "                                  ",
    "                                  ",
    "                                  ",
    "                                  ",
    "                                  ",
    "                                  ",
    "                                  ",
    "                     o            ",
    "                  o     o         ",
    "               o                  ",
    "        o      #######      o     ",
    "     o                       o    ",
    "  P        o          o   s     G ",
    "##################################",
    "##################################",
];

// 36-wide, with two-tile pits to jump and a platform over one of them.
const L2: &[&str] = &[
    "                                    ",
    "                                    ",
    "                                    ",
    "                                    ",
    "                                    ",
    "                                    ",
    "                                    ",
    "                                    ",
    "                                    ",
    "                  o                 ",
    "          o      #####       o      ",
    "      o                  o      o   ",
    "  P      s          o         s   G ",
    "######  ########  #######  #########",
    "######  ########  #######  #########",
];

const LEVELS: [&[&str]; 2] = [L1, L2];

struct Player {
    vel: Vec2,
    on_ground: bool,
}
struct Enemy {
    dir: f32,
    vel_y: f32,
}
struct Coin;
struct Goal;

/// Texture handles, loaded once and copied into every scene.
#[derive(Default, Clone, Copy)]
struct Res {
    ground: Texture,
    player: Texture,
    slime: Texture,
    coin: Texture,
    flag: Texture,
}

/// A level's solid-tile grid, used for collision (rendering uses entities).
struct Grid {
    solids: Vec<bool>,
    cols: i32,
    rows: i32,
}

impl Grid {
    fn from_level(level: &[&str]) -> Grid {
        let rows = level.len() as i32;
        let cols = level.iter().map(|l| l.chars().count()).max().unwrap_or(0) as i32;
        let mut solids = vec![false; (cols * rows) as usize];
        for (r, line) in level.iter().enumerate() {
            for (c, ch) in line.chars().enumerate() {
                if ch == '#' {
                    solids[r * cols as usize + c] = true;
                }
            }
        }
        Grid { solids, cols, rows }
    }

    fn solid(&self, c: i32, r: i32) -> bool {
        if c < 0 || r < 0 || c >= self.cols || r >= self.rows {
            false
        } else {
            self.solids[(r * self.cols + c) as usize]
        }
    }

    fn rows_solid(&self, rect: &Rect, col: i32) -> bool {
        let r0 = ((rect.y + 2.0) / TILE) as i32;
        let r1 = ((rect.y + rect.h - 2.0) / TILE) as i32;
        (r0..=r1).any(|r| self.solid(col, r))
    }

    fn cols_solid(&self, rect: &Rect, row: i32) -> bool {
        let c0 = ((rect.x + 2.0) / TILE) as i32;
        let c1 = ((rect.x + rect.w - 2.0) / TILE) as i32;
        (c0..=c1).any(|c| self.solid(c, row))
    }
}

/// Moves `rect` by `vel * dt`, resolving against solid tiles one axis at a
/// time. Returns whether the body is now standing on ground.
fn move_body(rect: &mut Rect, vel: &mut Vec2, dt: f32, grid: &Grid) -> bool {
    rect.x += vel.x * dt;
    if vel.x > 0.0 {
        let col = ((rect.x + rect.w) / TILE) as i32;
        if grid.rows_solid(rect, col) {
            rect.x = col as f32 * TILE - rect.w;
            vel.x = 0.0;
        }
    } else if vel.x < 0.0 {
        let col = (rect.x / TILE) as i32;
        if grid.rows_solid(rect, col) {
            rect.x = (col + 1) as f32 * TILE;
            vel.x = 0.0;
        }
    }

    rect.y += vel.y * dt;
    let mut on_ground = false;
    if vel.y > 0.0 {
        let row = ((rect.y + rect.h) / TILE) as i32;
        if grid.cols_solid(rect, row) {
            rect.y = row as f32 * TILE - rect.h;
            vel.y = 0.0;
            on_ground = true;
        }
    } else if vel.y < 0.0 {
        let row = (rect.y / TILE) as i32;
        if grid.cols_solid(rect, row) {
            rect.y = (row + 1) as f32 * TILE;
            vel.y = 0.0;
        }
    }
    on_ground
}

// ---------------------------------------------------------------------------
// Scenes
// ---------------------------------------------------------------------------

struct TitleScene {
    res: Res,
}

impl Scene for TitleScene {
    fn enter(&mut self, world: &mut World) {
        world.clear();
    }
    fn update(&mut self, _world: &mut World, frame: &Frame) -> Transition {
        if frame.input.is_key_pressed(Key::Space) || frame.input.is_key_pressed(Key::Enter) {
            Transition::Replace(Box::new(PlayScene::new(self.res, 0, 0, 3)))
        } else {
            Transition::None
        }
    }
    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        painter.text_centered(SCREEN.x * 0.5, 170.0, 1.7, [1.0, 1.0, 1.0, 1.0], "OCTIRON");
        painter.text_centered(SCREEN.x * 0.5, 250.0, 0.95, [1.0, 0.92, 0.5, 1.0], "PLATFORMER");
        painter.text_centered(
            SCREEN.x * 0.5,
            370.0,
            0.55,
            [0.92, 0.95, 1.0, 1.0],
            "ARROWS / WASD TO MOVE     SPACE TO JUMP",
        );
        painter.text_centered(SCREEN.x * 0.5, 430.0, 0.7, [0.7, 1.0, 0.7, 1.0], "PRESS SPACE TO START");
    }
}

struct EndScene {
    res: Res,
    won: bool,
    score: u32,
}

impl Scene for EndScene {
    fn enter(&mut self, world: &mut World) {
        world.clear();
    }
    fn update(&mut self, _world: &mut World, frame: &Frame) -> Transition {
        if frame.input.is_key_pressed(Key::Space) || frame.input.is_key_pressed(Key::Enter) {
            Transition::Replace(Box::new(TitleScene { res: self.res }))
        } else {
            Transition::None
        }
    }
    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        let (title, color) = if self.won {
            ("YOU WIN!", [0.6, 1.0, 0.7, 1.0])
        } else {
            ("GAME OVER", [1.0, 0.55, 0.55, 1.0])
        };
        painter.text_centered(SCREEN.x * 0.5, 230.0, 1.5, color, title);
        painter.text_centered(
            SCREEN.x * 0.5,
            320.0,
            0.7,
            [1.0, 0.9, 0.4, 1.0],
            &format!("COINS  {}", self.score),
        );
        painter.text_centered(SCREEN.x * 0.5, 410.0, 0.55, [0.85, 0.85, 0.95, 1.0], "PRESS SPACE");
    }
}

struct PlayScene {
    res: Res,
    level: usize,
    score: u32,
    lives: u32,
    grid: Grid,
    world_size: Vec2,
    camera: Vec2,
}

impl PlayScene {
    fn new(res: Res, level: usize, score: u32, lives: u32) -> PlayScene {
        let grid = Grid::from_level(LEVELS[level]);
        let world_size = Vec2::new(grid.cols as f32 * TILE, grid.rows as f32 * TILE);
        PlayScene {
            res,
            level,
            score,
            lives,
            grid,
            world_size,
            camera: Vec2::ZERO,
        }
    }
}

impl Scene for PlayScene {
    fn enter(&mut self, world: &mut World) {
        world.clear();
        for (r, line) in LEVELS[self.level].iter().enumerate() {
            for (c, ch) in line.chars().enumerate() {
                let x = c as f32 * TILE;
                let y = r as f32 * TILE;
                match ch {
                    '#' => {
                        world.spawn((
                            Transform::new(Vec2::new(x, y), Vec2::new(TILE, TILE)),
                            Sprite::texture(self.res.ground),
                        ));
                    }
                    'P' => {
                        let pos = Vec2::new(x + (TILE - PLAYER_SIZE.x) * 0.5, y + TILE - PLAYER_SIZE.y);
                        world.spawn((
                            Transform::new(pos, PLAYER_SIZE),
                            Sprite::texture(self.res.player),
                            Player {
                                vel: Vec2::ZERO,
                                on_ground: false,
                            },
                        ));
                    }
                    's' => {
                        let pos = Vec2::new(x + (TILE - ENEMY_SIZE.x) * 0.5, y + TILE - ENEMY_SIZE.y);
                        world.spawn((
                            Transform::new(pos, ENEMY_SIZE),
                            Sprite::texture(self.res.slime),
                            Enemy { dir: -1.0, vel_y: 0.0 },
                        ));
                    }
                    'o' => {
                        let pos = Vec2::new(x + (TILE - COIN_SIZE) * 0.5, y + (TILE - COIN_SIZE) * 0.5);
                        world.spawn((
                            Transform::new(pos, Vec2::new(COIN_SIZE, COIN_SIZE)),
                            Sprite::texture(self.res.coin),
                            Coin,
                        ));
                    }
                    'G' => {
                        let pos = Vec2::new(x + (TILE - FLAG_SIZE.x) * 0.5, y + TILE - FLAG_SIZE.y);
                        world.spawn((
                            Transform::new(pos, FLAG_SIZE),
                            Sprite::texture(self.res.flag),
                            Goal,
                        ));
                    }
                    _ => {}
                }
            }
        }
    }

    fn update(&mut self, world: &mut World, frame: &Frame) -> Transition {
        let dt = frame.dt.min(1.0 / 30.0);
        let jump = frame.input.is_key_pressed(Key::Space)
            || frame.input.is_key_pressed(Key::ArrowUp)
            || frame.input.is_key_pressed(Key::KeyW);
        let mut dir = 0.0;
        if frame.input.is_key_down(Key::ArrowLeft) || frame.input.is_key_down(Key::KeyA) {
            dir -= 1.0;
        }
        if frame.input.is_key_down(Key::ArrowRight) || frame.input.is_key_down(Key::KeyD) {
            dir += 1.0;
        }

        // Player.
        let mut player_rect = Rect::new(0.0, 0.0, PLAYER_SIZE.x, PLAYER_SIZE.y);
        let mut player_falling = false;
        let mut died = false;
        for (transform, player) in world.query_mut::<(&mut Transform, &mut Player)>() {
            player.vel.x = dir * MOVE_SPEED;
            if jump && player.on_ground {
                player.vel.y = -JUMP_SPEED;
                player.on_ground = false;
            }
            player.vel.y = (player.vel.y + GRAVITY * dt).min(MAX_FALL);
            let mut rect = transform.rect();
            player.on_ground = move_body(&mut rect, &mut player.vel, dt, &self.grid);
            rect.x = rect.x.clamp(0.0, self.world_size.x - rect.w);
            transform.position = Vec2::new(rect.x, rect.y);
            player_rect = transform.rect();
            player_falling = player.vel.y > 0.0;
            if transform.position.y > self.world_size.y + 80.0 {
                died = true;
            }
        }

        // Enemies: patrol, gravity, turn at walls and ledges.
        for (transform, enemy) in world.query_mut::<(&mut Transform, &mut Enemy)>() {
            let mut vel = Vec2::new(enemy.dir * ENEMY_SPEED, (enemy.vel_y + GRAVITY * dt).min(MAX_FALL));
            let mut rect = transform.rect();
            let on_ground = move_body(&mut rect, &mut vel, dt, &self.grid);
            if vel.x == 0.0 {
                enemy.dir = -enemy.dir;
            }
            if rect.x <= 0.0 || rect.x >= self.world_size.x - rect.w {
                enemy.dir = -enemy.dir;
                rect.x = rect.x.clamp(0.0, self.world_size.x - rect.w);
            }
            if on_ground {
                let front = if enemy.dir > 0.0 {
                    rect.x + rect.w + 2.0
                } else {
                    rect.x - 2.0
                };
                let below = ((rect.y + rect.h + 2.0) / TILE) as i32;
                if !self.grid.solid((front / TILE) as i32, below) {
                    enemy.dir = -enemy.dir;
                }
            }
            enemy.vel_y = vel.y;
            transform.position = Vec2::new(rect.x, rect.y);
        }

        // Coins.
        let coins: Vec<Entity> = world
            .query_mut::<(Entity, &Transform, &Coin)>()
            .into_iter()
            .filter(|(_, t, _)| player_rect.intersects(t.rect()))
            .map(|(e, _, _)| e)
            .collect();
        for e in coins {
            let _ = world.despawn(e);
            self.score += 1;
        }

        // Enemy contact: stomp from above, otherwise take damage.
        let enemies: Vec<(Entity, Rect)> = world
            .query_mut::<(Entity, &Transform, &Enemy)>()
            .into_iter()
            .map(|(e, t, _)| (e, t.rect()))
            .collect();
        let mut bounce = false;
        for (e, er) in &enemies {
            if player_rect.intersects(*er) {
                if player_falling && player_rect.center().y < er.center().y {
                    let _ = world.despawn(*e);
                    self.score += 2;
                    bounce = true;
                } else {
                    died = true;
                }
            }
        }
        if bounce {
            for player in world.query_mut::<&mut Player>() {
                player.vel.y = -JUMP_SPEED * 0.6;
            }
        }

        // Goal.
        let mut reached = false;
        for (transform, _goal) in world.query_mut::<(&Transform, &Goal)>() {
            if player_rect.intersects(transform.rect()) {
                reached = true;
            }
        }

        // Camera follows the player, clamped to the world.
        let center = player_rect.center();
        self.camera = Vec2::new(
            (center.x - SCREEN.x * 0.5).clamp(0.0, (self.world_size.x - SCREEN.x).max(0.0)),
            (center.y - SCREEN.y * 0.5).clamp(0.0, (self.world_size.y - SCREEN.y).max(0.0)),
        );

        if reached {
            return if self.level + 1 < LEVELS.len() {
                Transition::Replace(Box::new(PlayScene::new(
                    self.res,
                    self.level + 1,
                    self.score,
                    self.lives,
                )))
            } else {
                Transition::Replace(Box::new(EndScene {
                    res: self.res,
                    won: true,
                    score: self.score,
                }))
            };
        }
        if died {
            return if self.lives > 1 {
                Transition::Replace(Box::new(PlayScene::new(
                    self.res,
                    self.level,
                    self.score,
                    self.lives - 1,
                )))
            } else {
                Transition::Replace(Box::new(EndScene {
                    res: self.res,
                    won: false,
                    score: self.score,
                }))
            };
        }
        Transition::None
    }

    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        painter.text(16.0, 14.0, 0.5, [1.0, 0.9, 0.4, 1.0], &format!("COINS {}", self.score));
        painter.text(16.0, 44.0, 0.5, [1.0, 0.6, 0.6, 1.0], &format!("LIVES {}", self.lives));
        painter.text(
            SCREEN.x - 190.0,
            14.0,
            0.5,
            [0.75, 0.9, 1.0, 1.0],
            &format!("LEVEL {}", self.level + 1),
        );
    }

    fn camera(&self) -> Vec2 {
        self.camera
    }
}

// ---------------------------------------------------------------------------
// Game: drives the scene stack
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Platformer {
    res: Res,
    stack: Option<SceneStack>,
}

impl Game for Platformer {
    fn start(&mut self, _world: &mut World, assets: &mut Assets) {
        self.res = Res {
            ground: assets.load_png(include_bytes!("../assets/ground.png")),
            player: assets.load_png(include_bytes!("../assets/player.png")),
            slime: assets.load_png(include_bytes!("../assets/slime.png")),
            coin: assets.load_png(include_bytes!("../assets/coin.png")),
            flag: assets.load_png(include_bytes!("../assets/flag.png")),
        };
        self.stack = Some(SceneStack::new(Box::new(TitleScene { res: self.res })));
    }

    fn update(&mut self, world: &mut World, frame: &Frame) {
        if let Some(stack) = &mut self.stack {
            stack.update(world, frame);
        }
    }

    fn draw(&mut self, world: &World, painter: &mut Painter) {
        if let Some(stack) = &mut self.stack {
            stack.draw(world, painter);
        }
    }

    fn camera(&self) -> Vec2 {
        self.stack.as_ref().map(|s| s.camera()).unwrap_or(Vec2::ZERO)
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    run_with(
        Platformer::default(),
        Config {
            clear_color: [0.36, 0.62, 0.86, 1.0],
            logical_size: SCREEN,
        },
    );
}
