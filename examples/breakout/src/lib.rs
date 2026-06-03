//! Breakout, built on Octiron's ECS with textured bricks. Move the paddle with
//! A/D or ←/→. The ball launches upward; smash every brick to win, lose all
//! three lives and it's game over. Bricks are entities (Transform + Sprite +
//! Brick); the paddle and ball are entities too, so they auto-draw.

use octiron::{
    run, Assets, Entity, Frame, Game, Key, Painter, Rect, Sprite, Texture, Transform, Vec2, World,
};
use wasm_bindgen::prelude::*;

const FIELD: Vec2 = Vec2::new(800.0, 600.0);

const PADDLE: Vec2 = Vec2::new(120.0, 18.0);
const PADDLE_Y: f32 = 560.0;
const PADDLE_SPEED: f32 = 520.0;

const BALL: Vec2 = Vec2::new(16.0, 16.0);
const BALL_SPEED: f32 = 360.0;

const BRICK_COLS: usize = 10;
const BRICK_ROWS: usize = 6;
const BRICK_GAP: f32 = 4.0;
const FIELD_MARGIN: f32 = 24.0;
const BRICK_TOP: f32 = 60.0;
const BRICK_HEIGHT: f32 = 24.0;

const INITIAL_LIVES: u32 = 3;

/// Per-row brick tints (the source PNG is pale, so we colorize it).
const ROW_COLORS: [[f32; 4]; BRICK_ROWS] = [
    [0.94, 0.32, 0.36, 1.0],
    [0.96, 0.58, 0.27, 1.0],
    [0.97, 0.84, 0.33, 1.0],
    [0.46, 0.82, 0.44, 1.0],
    [0.36, 0.70, 0.95, 1.0],
    [0.66, 0.49, 0.93, 1.0],
];

struct Player;
struct Brick;

struct Ball {
    velocity: Vec2,
}

#[derive(PartialEq)]
enum State {
    Playing,
    Won,
    Lost,
}

struct Breakout {
    score: u32,
    lives: u32,
    state: State,
    brick: Texture,
    brick_count: u32,
}

impl Breakout {
    fn new() -> Self {
        Breakout {
            score: 0,
            lives: INITIAL_LIVES,
            state: State::Playing,
            brick: Texture::default(),
            brick_count: 0,
        }
    }

    fn brick_width() -> f32 {
        let usable = FIELD.x - FIELD_MARGIN * 2.0 - BRICK_GAP * (BRICK_COLS as f32 - 1.0);
        usable / BRICK_COLS as f32
    }

    /// Recenters the ball on top of the paddle and re-launches it upward.
    fn reset_ball(world: &mut World) {
        let mut paddle_cx = FIELD.x * 0.5;
        for (transform, _) in world.query_mut::<(&Transform, &Player)>() {
            paddle_cx = transform.position.x + transform.size.x * 0.5;
        }
        for (transform, ball) in world.query_mut::<(&mut Transform, &mut Ball)>() {
            transform.position = Vec2::new(paddle_cx - BALL.x * 0.5, PADDLE_Y - BALL.y - 10.0);
            ball.velocity = Vec2::new(BALL_SPEED * 0.45, -BALL_SPEED);
        }
    }
}

impl Default for Breakout {
    fn default() -> Self {
        Breakout::new()
    }
}

impl Game for Breakout {
    fn start(&mut self, world: &mut World, assets: &mut Assets) {
        self.brick = assets.load_png(include_bytes!("../assets/brick.png"));

        // Paddle.
        world.spawn((
            Transform::new(
                Vec2::new(FIELD.x * 0.5 - PADDLE.x * 0.5, PADDLE_Y),
                PADDLE,
            ),
            Sprite::color([0.42, 0.84, 1.0, 1.0]),
            Player,
        ));

        // Ball (launched upward, offset to the right).
        world.spawn((
            Transform::new(
                Vec2::new(FIELD.x * 0.5 - BALL.x * 0.5, PADDLE_Y - BALL.y - 10.0),
                BALL,
            ),
            Sprite::color([0.96, 0.96, 0.98, 1.0]),
            Ball {
                velocity: Vec2::new(BALL_SPEED * 0.45, -BALL_SPEED),
            },
        ));

        // Brick field.
        let bw = Breakout::brick_width();
        for row in 0..BRICK_ROWS {
            for col in 0..BRICK_COLS {
                let x = FIELD_MARGIN + col as f32 * (bw + BRICK_GAP);
                let y = BRICK_TOP + row as f32 * (BRICK_HEIGHT + BRICK_GAP);
                world.spawn((
                    Transform::new(Vec2::new(x, y), Vec2::new(bw, BRICK_HEIGHT)),
                    Sprite::texture(self.brick).tinted(ROW_COLORS[row]),
                    Brick,
                ));
                self.brick_count += 1;
            }
        }
    }

    fn update(&mut self, world: &mut World, frame: &Frame) {
        if self.state != State::Playing {
            return;
        }

        let dt = frame.dt;

        // Paddle movement (clamped on-screen).
        let left = frame.input.is_key_down(Key::ArrowLeft) || frame.input.is_key_down(Key::KeyA);
        let right = frame.input.is_key_down(Key::ArrowRight) || frame.input.is_key_down(Key::KeyD);
        let mut paddle_rect = Rect::new(FIELD.x * 0.5 - PADDLE.x * 0.5, PADDLE_Y, PADDLE.x, PADDLE.y);
        for (transform, _) in world.query_mut::<(&mut Transform, &Player)>() {
            let mut dir = 0.0;
            if left {
                dir -= 1.0;
            }
            if right {
                dir += 1.0;
            }
            transform.position.x = (transform.position.x + dir * PADDLE_SPEED * dt)
                .clamp(0.0, FIELD.x - transform.size.x);
            paddle_rect = transform.rect();
        }

        // Brick rects (snapshot for this frame's collision tests).
        let bricks: Vec<(Entity, Rect)> = world
            .query_mut::<(Entity, &Transform, &Brick)>()
            .into_iter()
            .map(|(e, t, _)| (e, t.rect()))
            .collect();

        let mut destroyed: Vec<Entity> = Vec::new();
        let mut lost_ball = false;

        // Move + collide the ball.
        for (transform, ball) in world.query_mut::<(&mut Transform, &mut Ball)>() {
            transform.position = transform.position + ball.velocity * dt;

            // Walls: left, right, top.
            if transform.position.x <= 0.0 {
                transform.position.x = 0.0;
                ball.velocity.x = ball.velocity.x.abs();
            } else if transform.position.x >= FIELD.x - BALL.x {
                transform.position.x = FIELD.x - BALL.x;
                ball.velocity.x = -ball.velocity.x.abs();
            }
            if transform.position.y <= 0.0 {
                transform.position.y = 0.0;
                ball.velocity.y = ball.velocity.y.abs();
            }

            let ball_rect = transform.rect();

            // Paddle: bounce up, with x steered by the contact offset.
            if ball.velocity.y > 0.0 && ball_rect.intersects(paddle_rect) {
                transform.position.y = paddle_rect.y - BALL.y;
                ball.velocity.y = -ball.velocity.y.abs();
                let offset =
                    (ball_rect.center().x - paddle_rect.center().x) / (paddle_rect.w * 0.5);
                ball.velocity.x = offset.clamp(-1.0, 1.0) * BALL_SPEED * 0.9;
            }

            // Bricks: bounce on the axis of least penetration, destroy one max
            // per frame so the bounce direction stays clean.
            let ball_rect = transform.rect();
            for (be, br) in &bricks {
                if !ball_rect.intersects(*br) {
                    continue;
                }
                let bc = ball_rect.center();
                let kc = br.center();
                // Overlap depth on each axis.
                let overlap_x = (ball_rect.w + br.w) * 0.5 - (bc.x - kc.x).abs();
                let overlap_y = (ball_rect.h + br.h) * 0.5 - (bc.y - kc.y).abs();
                if overlap_x < overlap_y {
                    if bc.x < kc.x {
                        transform.position.x -= overlap_x;
                        ball.velocity.x = -ball.velocity.x.abs();
                    } else {
                        transform.position.x += overlap_x;
                        ball.velocity.x = ball.velocity.x.abs();
                    }
                } else {
                    if bc.y < kc.y {
                        transform.position.y -= overlap_y;
                        ball.velocity.y = -ball.velocity.y.abs();
                    } else {
                        transform.position.y += overlap_y;
                        ball.velocity.y = ball.velocity.y.abs();
                    }
                }
                destroyed.push(*be);
                break;
            }

            // Fell off the bottom.
            if transform.position.y > FIELD.y {
                lost_ball = true;
            }
        }

        // Destroy hit bricks and score them.
        for e in destroyed {
            if world.despawn(e).is_ok() {
                self.score += 10;
                self.brick_count = self.brick_count.saturating_sub(1);
            }
        }

        if self.brick_count == 0 {
            self.state = State::Won;
            return;
        }

        if lost_ball {
            self.lives = self.lives.saturating_sub(1);
            if self.lives == 0 {
                self.state = State::Lost;
            } else {
                Breakout::reset_ball(world);
            }
        }
    }

    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        painter.text(
            16.0,
            14.0,
            0.6,
            [0.8, 0.92, 1.0, 1.0],
            &format!("SCORE {}", self.score),
        );
        let lives_text = format!("LIVES {}", self.lives);
        let w = painter.text_width(&lives_text, 0.6);
        painter.text(
            FIELD.x - 16.0 - w,
            14.0,
            0.6,
            [1.0, 0.78, 0.5, 1.0],
            &lives_text,
        );

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
    run(Breakout::new());
}
