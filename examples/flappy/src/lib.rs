//! Flappy Bird, built on Octiron's ECS with textured sprites. Press Space to
//! flap; thread the bird through the scrolling pipes without hitting a pipe,
//! the ground, or the ceiling. Press Space after a crash to restart.

use octiron::{
    run, Assets, Entity, Frame, Game, Key, Painter, Rect, Rng, Sprite, Texture, Transform, Vec2,
    World,
};
use wasm_bindgen::prelude::*;

const FIELD: Vec2 = Vec2::new(800.0, 600.0);

const BIRD: Vec2 = Vec2::new(34.0, 28.0);
const BIRD_X: f32 = 180.0;
const GRAVITY: f32 = 1500.0;
const FLAP_IMPULSE: f32 = -440.0;

const PIPE_W: f32 = 48.0;
const PIPE_GAP: f32 = 170.0;
const PIPE_SPEED: f32 = 170.0;
const PIPE_INTERVAL: f32 = 1.6;
const GAP_MARGIN: f32 = 70.0;

const GROUND_H: f32 = 64.0;
const GROUND_Y: f32 = FIELD.y - GROUND_H;

/// Marks the bird and carries its vertical velocity.
struct Bird {
    vy: f32,
}

/// One pipe segment (top or bottom). `scored` is shared semantically with its
/// partner via the gap; we flag the bottom pipe so a pair scores once.
struct Pipe {
    scored: bool,
    counts: bool,
}

#[derive(PartialEq)]
enum State {
    Playing,
    Over,
}

struct Flappy {
    state: State,
    score: u32,
    spawn_timer: f32,
    rng: Rng,
    bird_tex: Texture,
    pipe_tex: Texture,
}

impl Flappy {
    fn new() -> Self {
        Flappy {
            state: State::Playing,
            score: 0,
            spawn_timer: PIPE_INTERVAL,
            rng: Rng::new(0x9E37_79B9),
            bird_tex: Texture::default(),
            pipe_tex: Texture::default(),
        }
    }

    fn spawn_bird(&self, world: &mut World) {
        world.spawn((
            Transform::new(
                Vec2::new(BIRD_X, FIELD.y * 0.4 - BIRD.y * 0.5),
                BIRD,
            ),
            Sprite::texture(self.bird_tex),
            Bird { vy: 0.0 },
        ));
    }

    /// Spawns a top/bottom pipe pair with a randomized gap center.
    fn spawn_pipe_pair(&mut self, world: &mut World) {
        let min_center = GAP_MARGIN + PIPE_GAP * 0.5;
        let max_center = GROUND_Y - GAP_MARGIN - PIPE_GAP * 0.5;
        let gap_center = self.rng.range(min_center, max_center);

        let top_h = gap_center - PIPE_GAP * 0.5;
        let bottom_y = gap_center + PIPE_GAP * 0.5;
        let bottom_h = GROUND_Y - bottom_y;
        let x = FIELD.x;

        // Top pipe (stretched pipe.png).
        world.spawn((
            Transform::new(Vec2::new(x, 0.0), Vec2::new(PIPE_W, top_h)),
            Sprite::texture(self.pipe_tex),
            Pipe {
                scored: false,
                counts: false,
            },
        ));
        // Bottom pipe; this one tracks scoring for the pair.
        world.spawn((
            Transform::new(Vec2::new(x, bottom_y), Vec2::new(PIPE_W, bottom_h)),
            Sprite::texture(self.pipe_tex),
            Pipe {
                scored: false,
                counts: true,
            },
        ));
    }

    /// Removes every entity and rebuilds the bird for a fresh run.
    fn reset(&mut self, world: &mut World) {
        let all: Vec<Entity> = world
            .query_mut::<(Entity, &Transform)>()
            .into_iter()
            .map(|(e, _)| e)
            .collect();
        for e in all {
            let _ = world.despawn(e);
        }
        self.state = State::Playing;
        self.score = 0;
        self.spawn_timer = PIPE_INTERVAL;
        self.rng = Rng::new(0x9E37_79B9);
        self.spawn_bird(world);
    }
}

impl Game for Flappy {
    fn start(&mut self, world: &mut World, assets: &mut Assets) {
        self.bird_tex = assets.load_png(include_bytes!("../assets/bird.png"));
        self.pipe_tex = assets.load_png(include_bytes!("../assets/pipe.png"));
        self.spawn_bird(world);
    }

    fn update(&mut self, world: &mut World, frame: &Frame) {
        let dt = frame.dt;
        // Rising edge of Space (engine tracks the just-pressed state for us).
        let flap = frame.input.is_key_pressed(Key::Space);

        if self.state == State::Over {
            if flap {
                self.reset(world);
            }
            return;
        }

        // Bird physics: gravity + flap impulse on the press edge.
        let mut bird_rect: Option<Rect> = None;
        let mut crashed = false;
        for (transform, bird) in world.query_mut::<(&mut Transform, &mut Bird)>() {
            if flap {
                bird.vy = FLAP_IMPULSE;
            }
            bird.vy += GRAVITY * dt;
            transform.position.y += bird.vy * dt;

            // Ceiling clamp.
            if transform.position.y < 0.0 {
                transform.position.y = 0.0;
                bird.vy = 0.0;
            }
            // Ground collision.
            if transform.position.y + transform.size.y >= GROUND_Y {
                transform.position.y = GROUND_Y - transform.size.y;
                crashed = true;
            }
            bird_rect = Some(transform.rect());
        }

        // Spawn pipes on a timer.
        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 {
            self.spawn_timer += PIPE_INTERVAL;
            self.spawn_pipe_pair(world);
        }

        // Scroll pipes left.
        for (transform, _) in world.query_mut::<(&mut Transform, &Pipe)>() {
            transform.position.x -= PIPE_SPEED * dt;
        }

        // Scoring: when the bird passes a scoring pipe's right edge.
        if let Some(br) = bird_rect {
            let bird_left = br.x;
            for (transform, pipe) in world.query_mut::<(&Transform, &mut Pipe)>() {
                if pipe.counts && !pipe.scored && transform.position.x + transform.size.x < bird_left
                {
                    pipe.scored = true;
                    self.score += 1;
                }
            }
        }

        // Pipe collisions.
        if let Some(br) = bird_rect {
            for (transform, _) in world.query_mut::<(&Transform, &Pipe)>() {
                if br.intersects(transform.rect()) {
                    crashed = true;
                    break;
                }
            }
        }

        if crashed {
            self.state = State::Over;
        }

        // Despawn pipes that scrolled fully off the left edge.
        let gone: Vec<Entity> = world
            .query_mut::<(Entity, &Transform, &Pipe)>()
            .into_iter()
            .filter(|(_, t, _)| t.position.x + t.size.x < 0.0)
            .map(|(e, _, _)| e)
            .collect();
        for e in gone {
            let _ = world.despawn(e);
        }
    }

    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        // Ground strip along the bottom.
        painter.rect(0.0, GROUND_Y, FIELD.x, GROUND_H, [0.45, 0.33, 0.18, 1.0]);
        painter.rect(0.0, GROUND_Y, FIELD.x, 6.0, [0.55, 0.72, 0.28, 1.0]);

        // Score, centered near the top.
        painter.text_centered(
            FIELD.x * 0.5,
            28.0,
            1.2,
            [1.0, 1.0, 1.0, 1.0],
            &self.score.to_string(),
        );

        if self.state == State::Over {
            painter.text_centered(
                FIELD.x * 0.5,
                FIELD.y * 0.5 - 26.0,
                1.3,
                [1.0, 0.5, 0.5, 1.0],
                "GAME OVER",
            );
            painter.text_centered(
                FIELD.x * 0.5,
                FIELD.y * 0.5 + 18.0,
                0.6,
                [0.9, 0.9, 0.95, 1.0],
                "PRESS SPACE",
            );
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    run(Flappy::new());
}
