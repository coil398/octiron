//! Space Invaders, built on Octiron's ECS with textured sprites. Move with
//! A/D or ←/→, fire with Space. Clear the fleet to win; let it reach you and
//! it's game over. Reference example for `Assets::load_png` + textured sprites.

use octiron::{
    run, Assets, Entity, Frame, Game, Key, Painter, Rect, Rng, Sprite, Texture, Transform, Vec2,
    World,
};
use wasm_bindgen::prelude::*;

const FIELD: Vec2 = Vec2::new(800.0, 600.0);
const SHIP: Vec2 = Vec2::new(48.0, 36.0);
const SHIP_Y: f32 = 548.0;
const SHIP_SPEED: f32 = 430.0;
const BULLET: Vec2 = Vec2::new(8.0, 18.0);
const BULLET_SPEED: f32 = 580.0;
const SHOOT_CD: f32 = 0.32;
const ALIEN: Vec2 = Vec2::new(36.0, 28.0);
const ALIEN_COLS: usize = 9;
const ALIEN_ROWS: usize = 5;

struct Player;
struct Alien;
struct Bullet;
struct Star {
    speed: f32,
}

#[derive(PartialEq)]
enum State {
    Playing,
    Won,
    Lost,
}

struct Invaders {
    score: u32,
    shoot_cd: f32,
    fleet_dir: f32,
    fleet_speed: f32,
    state: State,
    tex: Tex,
}

#[derive(Default, Clone, Copy)]
struct Tex {
    ship: Texture,
    alien: Texture,
    alien2: Texture,
    bullet: Texture,
    star: Texture,
}

impl Default for Invaders {
    fn default() -> Self {
        Invaders {
            score: 0,
            shoot_cd: 0.0,
            fleet_dir: 1.0,
            fleet_speed: 46.0,
            state: State::Playing,
            tex: Tex::default(),
        }
    }
}

impl Game for Invaders {
    fn start(&mut self, world: &mut World, assets: &mut Assets) {
        self.tex = Tex {
            ship: assets.load_png(include_bytes!("../assets/ship.png")),
            alien: assets.load_png(include_bytes!("../assets/alien.png")),
            alien2: assets.load_png(include_bytes!("../assets/alien2.png")),
            bullet: assets.load_png(include_bytes!("../assets/bullet.png")),
            star: assets.load_png(include_bytes!("../assets/star.png")),
        };

        // Starfield.
        let mut rng = Rng::new(0x1234_5678);
        for _ in 0..70 {
            let size = rng.range(2.0, 5.0);
            world.spawn((
                Transform::new(
                    Vec2::new(rng.range(0.0, FIELD.x), rng.range(0.0, FIELD.y)),
                    Vec2::new(size, size),
                ),
                Sprite::texture(self.tex.star).tinted([1.0, 1.0, 1.0, rng.range(0.3, 0.9)]),
                Star {
                    speed: rng.range(18.0, 60.0),
                },
            ));
        }

        // Player.
        world.spawn((
            Transform::new(Vec2::new(FIELD.x * 0.5 - SHIP.x * 0.5, SHIP_Y), SHIP),
            Sprite::texture(self.tex.ship),
            Player,
        ));

        // Alien fleet.
        let gap = Vec2::new(54.0, 46.0);
        let origin = Vec2::new(
            (FIELD.x - (ALIEN_COLS as f32 - 1.0) * gap.x - ALIEN.x) * 0.5,
            70.0,
        );
        for row in 0..ALIEN_ROWS {
            for col in 0..ALIEN_COLS {
                let tex = if row % 2 == 0 {
                    self.tex.alien
                } else {
                    self.tex.alien2
                };
                world.spawn((
                    Transform::new(
                        Vec2::new(origin.x + col as f32 * gap.x, origin.y + row as f32 * gap.y),
                        ALIEN,
                    ),
                    Sprite::texture(tex),
                    Alien,
                ));
            }
        }
    }

    fn update(&mut self, world: &mut World, frame: &Frame) {
        let dt = frame.dt;

        // Stars drift even after the game ends, for ambiance.
        for (transform, star) in world.query_mut::<(&mut Transform, &Star)>() {
            transform.position.y += star.speed * dt;
            if transform.position.y > FIELD.y {
                transform.position.y = -transform.size.y;
            }
        }

        if self.state != State::Playing {
            return;
        }

        // Player movement + firing.
        self.shoot_cd -= dt;
        let left = frame.input.is_key_down(Key::ArrowLeft) || frame.input.is_key_down(Key::KeyA);
        let right = frame.input.is_key_down(Key::ArrowRight) || frame.input.is_key_down(Key::KeyD);
        let mut ship_top: Option<(f32, f32)> = None;
        for (transform, _) in world.query_mut::<(&mut Transform, &Player)>() {
            let mut dir = 0.0;
            if left {
                dir -= 1.0;
            }
            if right {
                dir += 1.0;
            }
            transform.position.x =
                (transform.position.x + dir * SHIP_SPEED * dt).clamp(0.0, FIELD.x - transform.size.x);
            ship_top = Some((transform.position.x + transform.size.x * 0.5, transform.position.y));
        }
        if frame.input.is_key_down(Key::Space) && self.shoot_cd <= 0.0 {
            if let Some((cx, top)) = ship_top {
                world.spawn((
                    Transform::new(Vec2::new(cx - BULLET.x * 0.5, top - BULLET.y), BULLET),
                    Sprite::texture(self.tex.bullet),
                    Bullet,
                ));
                self.shoot_cd = SHOOT_CD;
            }
        }

        // Bullets fly up.
        for (transform, _) in world.query_mut::<(&mut Transform, &Bullet)>() {
            transform.position.y -= BULLET_SPEED * dt;
        }
        let gone: Vec<Entity> = world
            .query_mut::<(Entity, &Transform, &Bullet)>()
            .into_iter()
            .filter(|(_, t, _)| t.position.y + BULLET.y < 0.0)
            .map(|(e, _, _)| e)
            .collect();
        for e in gone {
            let _ = world.despawn(e);
        }

        // Fleet movement.
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        let mut count = 0u32;
        for (t, _) in world.query_mut::<(&Transform, &Alien)>() {
            min_x = min_x.min(t.position.x);
            max_x = max_x.max(t.position.x + t.size.x);
            max_y = max_y.max(t.position.y + t.size.y);
            count += 1;
        }
        if count == 0 {
            self.state = State::Won;
            return;
        }
        let dx = self.fleet_dir * self.fleet_speed * dt;
        if min_x + dx < 8.0 || max_x + dx > FIELD.x - 8.0 {
            self.fleet_dir = -self.fleet_dir;
            self.fleet_speed += 6.0;
            for (t, _) in world.query_mut::<(&mut Transform, &Alien)>() {
                t.position.y += 20.0;
            }
        } else {
            for (t, _) in world.query_mut::<(&mut Transform, &Alien)>() {
                t.position.x += dx;
            }
        }
        if max_y >= SHIP_Y {
            self.state = State::Lost;
        }

        // Bullet vs alien collisions.
        let bullets: Vec<(Entity, Rect)> = world
            .query_mut::<(Entity, &Transform, &Bullet)>()
            .into_iter()
            .map(|(e, t, _)| (e, t.rect()))
            .collect();
        let aliens: Vec<(Entity, Rect)> = world
            .query_mut::<(Entity, &Transform, &Alien)>()
            .into_iter()
            .map(|(e, t, _)| (e, t.rect()))
            .collect();
        let mut dead: Vec<Entity> = Vec::new();
        for (be, br) in &bullets {
            for (ae, ar) in &aliens {
                if br.intersects(*ar) && !dead.contains(ae) && !dead.contains(be) {
                    dead.push(*be);
                    dead.push(*ae);
                    self.score += 10;
                    break;
                }
            }
        }
        for e in dead {
            let _ = world.despawn(e);
        }
    }

    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        painter.text(16.0, 14.0, 0.55, [0.75, 0.9, 1.0, 1.0], &format!("SCORE {}", self.score));
        match self.state {
            State::Won => painter.text_centered(
                FIELD.x * 0.5,
                FIELD.y * 0.5 - 20.0,
                1.3,
                [0.6, 1.0, 0.7, 1.0],
                "YOU WIN",
            ),
            State::Lost => painter.text_centered(
                FIELD.x * 0.5,
                FIELD.y * 0.5 - 20.0,
                1.3,
                [1.0, 0.5, 0.5, 1.0],
                "GAME OVER",
            ),
            State::Playing => {}
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    run(Invaders::default());
}
