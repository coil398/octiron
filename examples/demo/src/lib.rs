//! Pong, built on Octiron's ECS, with a text score HUD. Left paddle: W/S,
//! right paddle: ↑/↓. This is the reference example for solid sprites + text.

use octiron::{run, Assets, Frame, Game, Key, Painter, Rect, Sprite, Transform, Vec2, World};
use wasm_bindgen::prelude::*;

const FIELD: Vec2 = Vec2::new(800.0, 600.0);
const PADDLE: Vec2 = Vec2::new(16.0, 110.0);
const BALL: Vec2 = Vec2::new(16.0, 16.0);
const PADDLE_SPEED: f32 = 460.0;
const BALL_SPEED: f32 = 320.0;
const WALL_MARGIN: f32 = 32.0;

struct Paddle {
    up: Key,
    down: Key,
}

struct Ball {
    velocity: Vec2,
}

#[derive(Default)]
struct Pong {
    score_left: u32,
    score_right: u32,
}

impl Pong {
    fn centered_ball() -> Transform {
        Transform::new(
            Vec2::new(FIELD.x * 0.5 - BALL.x * 0.5, FIELD.y * 0.5 - BALL.y * 0.5),
            BALL,
        )
    }
}

impl Game for Pong {
    fn start(&mut self, world: &mut World, _assets: &mut Assets) {
        let paddle_y = FIELD.y * 0.5 - PADDLE.y * 0.5;

        world.spawn((
            Transform::new(Vec2::new(WALL_MARGIN, paddle_y), PADDLE),
            Sprite::color([0.32, 0.78, 1.0, 1.0]),
            Paddle {
                up: Key::KeyW,
                down: Key::KeyS,
            },
        ));
        world.spawn((
            Transform::new(Vec2::new(FIELD.x - WALL_MARGIN - PADDLE.x, paddle_y), PADDLE),
            Sprite::color([1.0, 0.42, 0.31, 1.0]),
            Paddle {
                up: Key::ArrowUp,
                down: Key::ArrowDown,
            },
        ));
        world.spawn((
            Pong::centered_ball(),
            Sprite::color([0.95, 0.95, 0.98, 1.0]),
            Ball {
                velocity: Vec2::new(-BALL_SPEED, BALL_SPEED * 0.55),
            },
        ));
    }

    fn update(&mut self, world: &mut World, frame: &Frame) {
        let dt = frame.dt;

        for (transform, paddle) in world.query_mut::<(&mut Transform, &Paddle)>() {
            let mut dir = 0.0;
            if frame.input.is_key_down(paddle.up) {
                dir -= 1.0;
            }
            if frame.input.is_key_down(paddle.down) {
                dir += 1.0;
            }
            transform.position.y += dir * PADDLE_SPEED * dt;
            transform.position.y = transform.position.y.clamp(0.0, FIELD.y - PADDLE.y);
        }

        let paddles: Vec<Rect> = world
            .query_mut::<(&Transform, &Paddle)>()
            .into_iter()
            .map(|(transform, _)| transform.rect())
            .collect();

        let mut scored: Option<bool> = None; // Some(true) => left scored
        for (transform, ball) in world.query_mut::<(&mut Transform, &mut Ball)>() {
            transform.position = transform.position + ball.velocity * dt;

            if transform.position.y <= 0.0 {
                transform.position.y = 0.0;
                ball.velocity.y = ball.velocity.y.abs();
            } else if transform.position.y >= FIELD.y - BALL.y {
                transform.position.y = FIELD.y - BALL.y;
                ball.velocity.y = -ball.velocity.y.abs();
            }

            let ball_rect = transform.rect();
            for paddle_rect in &paddles {
                if ball_rect.intersects(*paddle_rect) {
                    if ball.velocity.x < 0.0 {
                        transform.position.x = paddle_rect.x + paddle_rect.w;
                        ball.velocity.x = ball.velocity.x.abs();
                    } else {
                        transform.position.x = paddle_rect.x - BALL.x;
                        ball.velocity.x = -ball.velocity.x.abs();
                    }
                    let offset =
                        (ball_rect.center().y - paddle_rect.center().y) / (paddle_rect.h * 0.5);
                    ball.velocity.y += offset * 140.0;
                }
            }

            if transform.position.x < -BALL.x || transform.position.x > FIELD.x {
                let toward_left = transform.position.x > FIELD.x;
                scored = Some(toward_left);
                *transform = Pong::centered_ball();
                ball.velocity = Vec2::new(
                    if toward_left { -BALL_SPEED } else { BALL_SPEED },
                    BALL_SPEED * 0.55,
                );
            }
        }

        match scored {
            Some(true) => self.score_left += 1,
            Some(false) => self.score_right += 1,
            None => {}
        }
    }

    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        // Center net.
        let mut y = 8.0;
        while y < FIELD.y {
            painter.rect(FIELD.x * 0.5 - 2.0, y, 4.0, 16.0, [0.25, 0.28, 0.36, 1.0]);
            y += 28.0;
        }
        // Scores.
        let s = 1.1;
        painter.text(FIELD.x * 0.5 - 90.0, 24.0, s, [0.6, 0.85, 1.0, 1.0], &self.score_left.to_string());
        painter.text(FIELD.x * 0.5 + 60.0, 24.0, s, [1.0, 0.6, 0.5, 1.0], &self.score_right.to_string());
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    run(Pong::default());
}
