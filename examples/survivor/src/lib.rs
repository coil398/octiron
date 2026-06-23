//! Octiron Survivor — the big one. A top-down arena roguelite: roam a scrolling
//! world, weapons auto-fire at the nearest foe, swarms of three enemy types
//! close in, kills drop XP gems, and every level-up pauses for a choice of
//! upgrades. HP, a difficulty ramp, hit particles, a pause overlay, and a
//! game-over loop. Built on Octiron's ECS, camera, and scene stack.
//!
//! Move WASD / arrows · choose upgrades 1/2/3 · pause P.

use std::collections::HashMap;

use octiron::{
    run_with, Assets, Color, Config, Entity, Frame, Game, Key, Painter, Rect, Rng, Scene,
    SceneStack, Sprite, Texture, Transition, Transform, Vec2, World,
};
use wasm_bindgen::prelude::*;

const SCREEN: Vec2 = Vec2::new(800.0, 600.0);
const ARENA: Vec2 = Vec2::new(2200.0, 1600.0);
const FLOOR_TILE: f32 = 64.0;

const PLAYER_SIZE: Vec2 = Vec2::new(30.0, 34.0);
const GEM_SIZE: Vec2 = Vec2::new(16.0, 16.0);
const ORB_SIZE: Vec2 = Vec2::new(14.0, 14.0);

// Enemy kinds.
const ZOMBIE: u8 = 0;
const BAT: u8 = 1;
const BRUTE: u8 = 2;

struct Player {
    hp: f32,
    flash: f32,
}
struct Enemy {
    kind: u8,
    hp: f32,
    speed: f32,
    damage: f32,
    touch: f32,
    flash: f32,
}
struct Bullet {
    vel: Vec2,
    damage: f32,
    life: f32,
    pierce: i32,
}
struct Gem {
    value: u32,
}
struct Particle {
    vel: Vec2,
    life: f32,
    max_life: f32,
}

#[derive(Default, Clone, Copy)]
struct Res {
    floor: Texture,
    hero: Texture,
    zombie: Texture,
    bat: Texture,
    brute: Texture,
    gem: Texture,
    orb: Texture,
}

impl Res {
    fn enemy_tex(&self, kind: u8) -> Texture {
        match kind {
            BAT => self.bat,
            BRUTE => self.brute,
            _ => self.zombie,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Upgrade {
    Damage,
    FireRate,
    Projectile,
    Speed,
    MaxHp,
    Pickup,
    BulletSpeed,
    Pierce,
}

const ALL_UPGRADES: [Upgrade; 8] = [
    Upgrade::Damage,
    Upgrade::FireRate,
    Upgrade::Projectile,
    Upgrade::Speed,
    Upgrade::MaxHp,
    Upgrade::Pickup,
    Upgrade::BulletSpeed,
    Upgrade::Pierce,
];

impl Upgrade {
    fn name(self) -> &'static str {
        match self {
            Upgrade::Damage => "POWER",
            Upgrade::FireRate => "RAPID FIRE",
            Upgrade::Projectile => "MULTISHOT",
            Upgrade::Speed => "SWIFT",
            Upgrade::MaxHp => "VITALITY",
            Upgrade::Pickup => "MAGNET",
            Upgrade::BulletSpeed => "VELOCITY",
            Upgrade::Pierce => "PIERCE",
        }
    }
    fn desc(self) -> &'static str {
        match self {
            Upgrade::Damage => "+25% damage",
            Upgrade::FireRate => "+22% fire rate",
            Upgrade::Projectile => "+1 projectile",
            Upgrade::Speed => "+15% move speed",
            Upgrade::MaxHp => "+25 max HP, heal",
            Upgrade::Pickup => "+35% pickup range",
            Upgrade::BulletSpeed => "+20% shot speed",
            Upgrade::Pierce => "shots pierce +1",
        }
    }
}

enum Mode {
    Playing,
    LevelUp([Upgrade; 3]),
}

// ---------------------------------------------------------------------------

struct Title {
    res: Res,
}

impl Scene for Title {
    fn enter(&mut self, world: &mut World) {
        world.clear();
    }
    fn update(&mut self, _world: &mut World, frame: &Frame) -> Transition {
        if frame.input.is_key_pressed(Key::Space) || frame.input.is_key_pressed(Key::Enter) {
            Transition::Replace(Box::new(PlayScene::new(self.res)))
        } else {
            Transition::None
        }
    }
    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        painter.text_centered(SCREEN.x * 0.5, 150.0, 1.7, [1.0, 1.0, 1.0, 1.0], "OCTIRON");
        painter.text_centered(SCREEN.x * 0.5, 230.0, 1.0, [1.0, 0.6, 0.45, 1.0], "SURVIVOR");
        painter.text_centered(
            SCREEN.x * 0.5,
            340.0,
            0.5,
            [0.9, 0.92, 1.0, 1.0],
            "WASD / ARROWS TO MOVE   WEAPONS AUTO-FIRE",
        );
        painter.text_centered(
            SCREEN.x * 0.5,
            385.0,
            0.5,
            [0.9, 0.92, 1.0, 1.0],
            "SURVIVE  ·  COLLECT GEMS  ·  LEVEL UP",
        );
        painter.text_centered(SCREEN.x * 0.5, 460.0, 0.7, [0.7, 1.0, 0.7, 1.0], "PRESS SPACE TO START");
    }
}

struct GameOver {
    res: Res,
    time: f32,
    kills: u32,
    level: u32,
}

impl Scene for GameOver {
    fn enter(&mut self, world: &mut World) {
        world.clear();
    }
    fn update(&mut self, _world: &mut World, frame: &Frame) -> Transition {
        if frame.input.is_key_pressed(Key::Space) || frame.input.is_key_pressed(Key::Enter) {
            Transition::Replace(Box::new(Title { res: self.res }))
        } else {
            Transition::None
        }
    }
    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        painter.text_centered(SCREEN.x * 0.5, 180.0, 1.5, [1.0, 0.5, 0.5, 1.0], "YOU DIED");
        let m = (self.time as u32) / 60;
        let s = (self.time as u32) % 60;
        painter.text_centered(
            SCREEN.x * 0.5,
            270.0,
            0.7,
            [1.0, 0.92, 0.5, 1.0],
            &format!("SURVIVED  {:02}:{:02}", m, s),
        );
        painter.text_centered(
            SCREEN.x * 0.5,
            315.0,
            0.6,
            [0.85, 0.95, 1.0, 1.0],
            &format!("KILLS {}    LEVEL {}", self.kills, self.level),
        );
        painter.text_centered(SCREEN.x * 0.5, 410.0, 0.55, [0.85, 0.85, 0.95, 1.0], "PRESS SPACE");
    }
}

struct Pause;

impl Scene for Pause {
    fn overlay(&self) -> bool {
        true
    }
    fn update(&mut self, _world: &mut World, frame: &Frame) -> Transition {
        if frame.input.is_key_pressed(Key::KeyP) || frame.input.is_key_pressed(Key::Escape) {
            Transition::Pop
        } else {
            Transition::None
        }
    }
    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        painter.rect(0.0, 0.0, SCREEN.x, SCREEN.y, [0.0, 0.0, 0.0, 0.55]);
        painter.text_centered(SCREEN.x * 0.5, 260.0, 1.2, [1.0, 1.0, 1.0, 1.0], "PAUSED");
        painter.text_centered(SCREEN.x * 0.5, 330.0, 0.55, [0.85, 0.9, 1.0, 1.0], "PRESS P TO RESUME");
    }
}

struct PlayScene {
    res: Res,
    rng: Rng,
    // weapon / player stats
    damage: f32,
    fire_interval: f32,
    fire_timer: f32,
    projectiles: i32,
    move_speed: f32,
    max_hp: f32,
    pickup_radius: f32,
    bullet_speed: f32,
    pierce: i32,
    // run state
    cur_hp: f32,
    time: f32,
    score: u32,
    kills: u32,
    xp: u32,
    level: u32,
    xp_next: u32,
    spawn_timer: f32,
    camera: Vec2,
    mode: Mode,
}

impl PlayScene {
    fn new(res: Res) -> PlayScene {
        PlayScene {
            res,
            rng: Rng::new(0x2BAD_F00D),
            damage: 10.0,
            fire_interval: 0.5,
            fire_timer: 0.0,
            projectiles: 1,
            move_speed: 205.0,
            max_hp: 100.0,
            pickup_radius: 140.0,
            bullet_speed: 470.0,
            pierce: 0,
            cur_hp: 100.0,
            time: 0.0,
            score: 0,
            kills: 0,
            xp: 0,
            level: 1,
            xp_next: 3,
            spawn_timer: 0.5,
            camera: Vec2::ZERO,
            mode: Mode::Playing,
        }
    }

    fn roll_upgrades(&mut self) -> [Upgrade; 3] {
        let mut chosen = [Upgrade::Damage; 3];
        let mut i = 0;
        while i < 3 {
            let u = ALL_UPGRADES[self.rng.below(ALL_UPGRADES.len() as i32) as usize];
            if !chosen[..i].contains(&u) {
                chosen[i] = u;
                i += 1;
            }
        }
        chosen
    }

    fn apply_upgrade(&mut self, up: Upgrade, world: &mut World) {
        match up {
            Upgrade::Damage => self.damage *= 1.25,
            Upgrade::FireRate => self.fire_interval *= 0.82,
            Upgrade::Projectile => self.projectiles += 1,
            Upgrade::Speed => self.move_speed *= 1.15,
            Upgrade::MaxHp => {
                self.max_hp += 25.0;
                for player in world.query_mut::<&mut Player>() {
                    player.hp = (player.hp + 25.0).min(self.max_hp);
                }
            }
            Upgrade::Pickup => self.pickup_radius *= 1.35,
            Upgrade::BulletSpeed => self.bullet_speed *= 1.2,
            Upgrade::Pierce => self.pierce += 1,
        }
    }

    fn spawn_enemy(&mut self, world: &mut World, player_pos: Vec2) {
        let angle = self.rng.range(0.0, std::f32::consts::TAU);
        let radius = 560.0;
        let mut pos = Vec2::new(
            player_pos.x + angle.cos() * radius,
            player_pos.y + angle.sin() * radius,
        );
        pos.x = pos.x.clamp(0.0, ARENA.x);
        pos.y = pos.y.clamp(0.0, ARENA.y);

        let roll = self.rng.next_f32();
        let brute_chance = (self.time * 0.004).min(0.28);
        let bat_chance = 0.35;
        let kind = if roll < brute_chance {
            BRUTE
        } else if roll < brute_chance + bat_chance {
            BAT
        } else {
            ZOMBIE
        };
        let scale = 1.0 + self.time * 0.018;
        let (size, hp, speed, damage) = match kind {
            BAT => (Vec2::new(34.0, 20.0), 12.0 * scale, 132.0, 5.0),
            BRUTE => (Vec2::new(46.0, 42.0), 90.0 * scale, 48.0, 16.0),
            _ => (Vec2::new(30.0, 32.0), 28.0 * scale, 74.0, 8.0),
        };
        world.spawn((
            Transform::new(pos, size),
            Sprite::texture(self.res.enemy_tex(kind)),
            Enemy {
                kind,
                hp,
                speed,
                damage,
                touch: 0.0,
                flash: 0.0,
            },
        ));
    }
}

impl Scene for PlayScene {
    fn enter(&mut self, world: &mut World) {
        world.clear();
        // Floor tiles (static, drawn under everything via the camera).
        let cols = (ARENA.x / FLOOR_TILE).ceil() as i32;
        let rows = (ARENA.y / FLOOR_TILE).ceil() as i32;
        for r in 0..rows {
            for c in 0..cols {
                world.spawn((
                    Transform::new(
                        Vec2::new(c as f32 * FLOOR_TILE, r as f32 * FLOOR_TILE),
                        Vec2::new(FLOOR_TILE, FLOOR_TILE),
                    ),
                    Sprite::texture(self.res.floor),
                ));
            }
        }
        // Player at the center.
        world.spawn((
            Transform::new(
                Vec2::new(ARENA.x * 0.5 - PLAYER_SIZE.x * 0.5, ARENA.y * 0.5 - PLAYER_SIZE.y * 0.5),
                PLAYER_SIZE,
            ),
            Sprite::texture(self.res.hero),
            Player {
                hp: self.max_hp,
                flash: 0.0,
            },
        ));
        // An opening wave so the arena starts busy.
        let center = Vec2::new(ARENA.x * 0.5, ARENA.y * 0.5);
        for _ in 0..6 {
            self.spawn_enemy(world, center);
        }
    }

    fn update(&mut self, world: &mut World, frame: &Frame) -> Transition {
        let dt = frame.dt.min(0.05);
        let input = frame.input;

        // Level-up choice freezes the action.
        if let Mode::LevelUp(choices) = self.mode {
            let pick = if input.is_key_pressed(Key::Digit1) {
                Some(0)
            } else if input.is_key_pressed(Key::Digit2) {
                Some(1)
            } else if input.is_key_pressed(Key::Digit3) {
                Some(2)
            } else {
                None
            };
            if let Some(i) = pick {
                self.apply_upgrade(choices[i], world);
                self.mode = Mode::Playing;
            }
            return Transition::None;
        }

        if input.is_key_pressed(Key::KeyP) || input.is_key_pressed(Key::Escape) {
            return Transition::Push(Box::new(Pause));
        }

        self.time += dt;

        // Player movement.
        let mut dir = Vec2::ZERO;
        if input.is_key_down(Key::ArrowLeft) || input.is_key_down(Key::KeyA) {
            dir.x -= 1.0;
        }
        if input.is_key_down(Key::ArrowRight) || input.is_key_down(Key::KeyD) {
            dir.x += 1.0;
        }
        if input.is_key_down(Key::ArrowUp) || input.is_key_down(Key::KeyW) {
            dir.y -= 1.0;
        }
        if input.is_key_down(Key::ArrowDown) || input.is_key_down(Key::KeyS) {
            dir.y += 1.0;
        }
        let step = dir.normalized() * (self.move_speed * dt);
        let mut player_pos = Vec2::ZERO;
        let mut player_hp = 0.0;
        for (transform, player) in world.query_mut::<(&mut Transform, &mut Player)>() {
            transform.position = transform.position + step;
            transform.position.x = transform.position.x.clamp(0.0, ARENA.x - transform.size.x);
            transform.position.y = transform.position.y.clamp(0.0, ARENA.y - transform.size.y);
            player.flash = (player.flash - dt).max(0.0);
            player_pos = transform.rect().center();
            player_hp = player.hp;
        }

        // Camera follows the player, clamped to the arena.
        self.camera = Vec2::new(
            (player_pos.x - SCREEN.x * 0.5).clamp(0.0, (ARENA.x - SCREEN.x).max(0.0)),
            (player_pos.y - SCREEN.y * 0.5).clamp(0.0, (ARENA.y - SCREEN.y).max(0.0)),
        );

        // Spawn enemies (faster and thicker over time, capped).
        let enemy_count = world.query_mut::<&Enemy>().into_iter().count();
        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 && enemy_count < 260 {
            self.spawn_timer = (0.55 - self.time * 0.012).max(0.14);
            let batch = 2 + (self.time / 6.0) as i32;
            for _ in 0..batch {
                self.spawn_enemy(world, player_pos);
            }
        }

        // Enemy AI: home in on the player, track the nearest for aiming, deal
        // contact damage. Damage to the player is accumulated then applied.
        let mut damage_to_player = 0.0;
        let mut nearest: Option<(f32, Vec2)> = None;
        let player_rect = Rect::new(
            player_pos.x - PLAYER_SIZE.x * 0.5,
            player_pos.y - PLAYER_SIZE.y * 0.5,
            PLAYER_SIZE.x,
            PLAYER_SIZE.y,
        );
        for (transform, enemy) in world.query_mut::<(&mut Transform, &mut Enemy)>() {
            let center = transform.rect().center();
            let to_player = player_pos - center;
            let d2 = to_player.dot(to_player);
            transform.position = transform.position + to_player.normalized() * (enemy.speed * dt);
            enemy.touch = (enemy.touch - dt).max(0.0);
            enemy.flash = (enemy.flash - dt).max(0.0);
            if nearest.map_or(true, |(bd, _)| d2 < bd) {
                nearest = Some((d2, center));
            }
            if enemy.touch <= 0.0 && transform.rect().intersects(player_rect) {
                damage_to_player += enemy.damage;
                enemy.touch = 0.5;
            }
        }

        if damage_to_player > 0.0 {
            for player in world.query_mut::<&mut Player>() {
                player.hp -= damage_to_player;
                player.flash = 0.15;
                player_hp = player.hp;
            }
        }
        self.cur_hp = player_hp.max(0.0);
        if player_hp <= 0.0 {
            return Transition::Replace(Box::new(GameOver {
                res: self.res,
                time: self.time,
                kills: self.kills,
                level: self.level,
            }));
        }

        // Weapons auto-fire at the nearest enemy.
        self.fire_timer -= dt;
        if self.fire_timer <= 0.0 {
            if let Some((_, target)) = nearest {
                self.fire_timer = self.fire_interval;
                let base = (target - player_pos).normalized();
                let base_angle = base.y.atan2(base.x);
                let spread = 0.20;
                for i in 0..self.projectiles {
                    let offset = (i as f32 - (self.projectiles as f32 - 1.0) * 0.5) * spread;
                    let a = base_angle + offset;
                    let vel = Vec2::new(a.cos(), a.sin()) * self.bullet_speed;
                    world.spawn((
                        Transform::new(player_pos - ORB_SIZE * 0.5, ORB_SIZE),
                        Sprite::texture(self.res.orb),
                        Bullet {
                            vel,
                            damage: self.damage,
                            life: 1.3,
                            pierce: self.pierce,
                        },
                    ));
                }
            }
        }

        // Move bullets; expire by lifetime.
        let mut dead_bullets: Vec<Entity> = Vec::new();
        for (e, transform, bullet) in world.query_mut::<(Entity, &mut Transform, &mut Bullet)>() {
            transform.position = transform.position + bullet.vel * dt;
            bullet.life -= dt;
            if bullet.life <= 0.0 {
                dead_bullets.push(e);
            }
        }

        // Bullet -> enemy collisions.
        let bullets: Vec<(Entity, Rect, f32, i32)> = world
            .query_mut::<(Entity, &Transform, &Bullet)>()
            .into_iter()
            .map(|(e, t, b)| (e, t.rect(), b.damage, b.pierce))
            .collect();
        let enemies: Vec<(Entity, Rect)> = world
            .query_mut::<(Entity, &Transform, &Enemy)>()
            .into_iter()
            .map(|(e, t, _)| (e, t.rect()))
            .collect();
        let mut hp_loss: HashMap<Entity, f32> = HashMap::new();
        for (be, brect, dmg, mut pierce) in bullets {
            if dead_bullets.contains(&be) {
                continue;
            }
            for (ee, erect) in &enemies {
                if brect.intersects(*erect) {
                    *hp_loss.entry(*ee).or_insert(0.0) += dmg;
                    pierce -= 1;
                    if pierce < 0 {
                        dead_bullets.push(be);
                        break;
                    }
                }
            }
        }

        // Apply damage; collect kills.
        let mut deaths: Vec<(Vec2, u32, Color)> = Vec::new();
        for (e, transform, enemy) in world.query_mut::<(Entity, &mut Transform, &mut Enemy)>() {
            if let Some(loss) = hp_loss.get(&e) {
                enemy.hp -= loss;
                enemy.flash = 0.08;
            }
            if enemy.hp <= 0.0 {
                let value = match enemy.kind {
                    BRUTE => 5,
                    _ => 1,
                };
                let color = match enemy.kind {
                    BAT => [0.6, 0.4, 0.8, 1.0],
                    BRUTE => [0.9, 0.4, 0.4, 1.0],
                    _ => [0.5, 0.8, 0.4, 1.0],
                };
                deaths.push((transform.rect().center(), value, color));
            }
        }

        // Despawn dead enemies (re-scan, since we couldn't despawn mid-iteration).
        let to_kill: Vec<Entity> = world
            .query_mut::<(Entity, &Enemy)>()
            .into_iter()
            .filter(|(_, en)| en.hp <= 0.0)
            .map(|(e, _)| e)
            .collect();
        for e in to_kill {
            let _ = world.despawn(e);
        }
        for e in dead_bullets {
            let _ = world.despawn(e);
        }

        // Spawn gems + particles for each kill.
        for (pos, value, color) in deaths {
            self.score += value;
            self.kills += 1;
            world.spawn((
                Transform::new(pos - GEM_SIZE * 0.5, GEM_SIZE),
                Sprite::texture(self.res.gem),
                Gem { value },
            ));
            for _ in 0..6 {
                let a = self.rng.range(0.0, std::f32::consts::TAU);
                let sp = self.rng.range(40.0, 160.0);
                world.spawn((
                    Transform::new(pos, Vec2::new(6.0, 6.0)),
                    Sprite::color(color),
                    Particle {
                        vel: Vec2::new(a.cos() * sp, a.sin() * sp),
                        life: 0.4,
                        max_life: 0.4,
                    },
                ));
            }
        }

        // Gems: magnet toward the player, collect on contact.
        let pr2 = self.pickup_radius * self.pickup_radius;
        let mut collected: Vec<(Entity, u32)> = Vec::new();
        for (e, transform, gem) in world.query_mut::<(Entity, &mut Transform, &Gem)>() {
            let center = transform.rect().center();
            let to_player = player_pos - center;
            let d2 = to_player.dot(to_player);
            if d2 < pr2 {
                transform.position = transform.position + to_player.normalized() * (360.0 * dt);
            }
            if d2 < 26.0 * 26.0 {
                collected.push((e, gem.value));
            }
        }
        for (e, value) in collected {
            let _ = world.despawn(e);
            self.xp += value;
        }

        // Particles: drift, fade, expire.
        let mut dead_particles: Vec<Entity> = Vec::new();
        for (e, transform, sprite, particle) in
            world.query_mut::<(Entity, &mut Transform, &mut Sprite, &mut Particle)>()
        {
            transform.position = transform.position + particle.vel * dt;
            particle.vel = particle.vel * 0.9;
            particle.life -= dt;
            sprite.color[3] = (particle.life / particle.max_life).max(0.0);
            if particle.life <= 0.0 {
                dead_particles.push(e);
            }
        }
        for e in dead_particles {
            let _ = world.despawn(e);
        }

        // Level up.
        if self.xp >= self.xp_next {
            self.xp -= self.xp_next;
            self.level += 1;
            self.xp_next = (self.xp_next as f32 * 1.35).ceil() as u32;
            let choices = self.roll_upgrades();
            self.mode = Mode::LevelUp(choices);
        }

        Transition::None
    }

    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        // XP bar across the top.
        let xpw = SCREEN.x * (self.xp as f32 / self.xp_next as f32).clamp(0.0, 1.0);
        painter.rect(0.0, 0.0, SCREEN.x, 8.0, [0.12, 0.14, 0.2, 1.0]);
        painter.rect(0.0, 0.0, xpw, 8.0, [0.4, 0.85, 1.0, 1.0]);

        // Timer + level + score.
        let m = (self.time as u32) / 60;
        let s = (self.time as u32) % 60;
        painter.text_centered(SCREEN.x * 0.5, 18.0, 0.6, [1.0, 1.0, 1.0, 1.0], &format!("{:02}:{:02}", m, s));
        painter.text(16.0, 18.0, 0.5, [0.7, 0.9, 1.0, 1.0], &format!("LV {}", self.level));

        // HP bar (top-left).
        let hpw = 190.0;
        let frac = (self.cur_hp / self.max_hp).clamp(0.0, 1.0);
        painter.rect(16.0, 46.0, hpw, 16.0, [0.26, 0.10, 0.10, 1.0]);
        painter.rect(16.0, 46.0, hpw * frac, 16.0, [0.88, 0.30, 0.30, 1.0]);
        painter.text(
            22.0,
            64.0,
            0.4,
            [1.0, 0.88, 0.88, 1.0],
            &format!("{} / {}", self.cur_hp.max(0.0) as i32, self.max_hp as i32),
        );
        painter.text(
            SCREEN.x - 170.0,
            18.0,
            0.5,
            [1.0, 0.9, 0.4, 1.0],
            &format!("KILLS {}", self.kills),
        );

        if let Mode::LevelUp(choices) = self.mode {
            painter.rect(0.0, 0.0, SCREEN.x, SCREEN.y, [0.0, 0.0, 0.0, 0.6]);
            painter.text_centered(SCREEN.x * 0.5, 110.0, 1.2, [1.0, 0.92, 0.4, 1.0], "LEVEL UP!");
            painter.text_centered(SCREEN.x * 0.5, 175.0, 0.5, [0.85, 0.9, 1.0, 1.0], "CHOOSE AN UPGRADE");
            for (i, up) in choices.iter().enumerate() {
                let y = 230.0 + i as f32 * 105.0;
                painter.rect(140.0, y, 520.0, 88.0, [0.12, 0.15, 0.22, 1.0]);
                painter.rect(140.0, y, 8.0, 88.0, [0.4, 0.85, 1.0, 1.0]);
                painter.text(168.0, y + 16.0, 0.7, [1.0, 1.0, 1.0, 1.0], &format!("{}  {}", i + 1, up.name()));
                painter.text(168.0, y + 52.0, 0.45, [0.8, 0.85, 0.95, 1.0], up.desc());
            }
        }
    }

    fn camera(&self) -> Vec2 {
        self.camera
    }
}

// HP bar needs the player's current HP; draw it from a tiny query helper in the
// game wrapper, which has the world. (Kept simple: the wrapper draws it.)

#[derive(Default)]
struct Survivor {
    res: Res,
    stack: Option<SceneStack>,
}

impl Game for Survivor {
    fn start(&mut self, _world: &mut World, assets: &mut Assets) {
        self.res = Res {
            floor: assets.load_png(include_bytes!("../assets/floor.png")),
            hero: assets.load_png(include_bytes!("../assets/hero.png")),
            zombie: assets.load_png(include_bytes!("../assets/zombie.png")),
            bat: assets.load_png(include_bytes!("../assets/bat.png")),
            brute: assets.load_png(include_bytes!("../assets/brute.png")),
            gem: assets.load_png(include_bytes!("../assets/gem.png")),
            orb: assets.load_png(include_bytes!("../assets/orb.png")),
        };
        self.stack = Some(SceneStack::new(Box::new(Title { res: self.res })));
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
    fn camera(&self) -> Vec2 {
        self.stack.as_ref().map(|s| s.camera()).unwrap_or(Vec2::ZERO)
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    run_with(
        Survivor::default(),
        Config {
            clear_color: [0.16, 0.20, 0.18, 1.0],
            logical_size: SCREEN,
        },
    );
}
