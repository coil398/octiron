//! A scene stack for stateful games: title screens, levels, pause overlays,
//! win/lose screens — each a self-contained [`Scene`] that can hand control to
//! another via a [`Transition`].
//!
//! A scene owns its own logic; entities live in the shared [`World`]. Scenes
//! carry whatever resources they need (texture handles are `Copy`, so pass them
//! into each scene you construct). Drive a stack from a [`Game`](crate::Game):
//!
//! ```no_run
//! use octiron::{Frame, Painter, Scene, SceneStack, Transition, World};
//!
//! struct Title;
//! impl Scene for Title {
//!     fn update(&mut self, _world: &mut World, _frame: &Frame) -> Transition {
//!         Transition::None
//!     }
//!     fn draw(&mut self, _world: &World, painter: &mut Painter) {
//!         painter.text(20.0, 20.0, 1.0, [1.0; 4], "PRESS START");
//!     }
//! }
//!
//! # fn make() -> SceneStack {
//! SceneStack::new(Box::new(Title))
//! # }
//! ```

use crate::math::Vec2;
use crate::{Frame, Painter, World};

/// What a [`Scene`] asks the engine to do after a frame.
pub enum Transition {
    /// Keep running this scene.
    None,
    /// Suspend this scene and run a new one on top (e.g. a pause overlay).
    Push(Box<dyn Scene>),
    /// Replace this scene with a new one (e.g. advance to the next level).
    Replace(Box<dyn Scene>),
    /// Drop this scene and resume the one beneath it.
    Pop,
}

/// One game state: a level, a menu, an overlay.
pub trait Scene {
    /// Called once when the scene becomes active, before its first `update`.
    /// Spawn entities here (call `world.clear()` first if you want a fresh
    /// world for a new level).
    fn enter(&mut self, world: &mut World) {
        let _ = world;
    }

    /// Advances the scene by one frame and chooses what happens next.
    fn update(&mut self, world: &mut World, frame: &Frame) -> Transition;

    /// Draws this scene's HUD/overlay. World entities are drawn automatically.
    fn draw(&mut self, world: &World, painter: &mut Painter);

    /// World-space camera offset for this scene (see [`Game::camera`](crate::Game::camera)).
    fn camera(&self) -> Vec2 {
        Vec2::ZERO
    }

    /// Uniform zoom factor for this scene (see [`Game::camera_zoom`](crate::Game::camera_zoom)).
    /// Default `1.0` (no zoom).
    fn camera_zoom(&self) -> f32 {
        1.0
    }

    /// If `true`, the scene beneath this one is still drawn (e.g. a translucent
    /// pause menu over a frozen level). Only the top scene ever updates.
    fn overlay(&self) -> bool {
        false
    }
}

/// A stack of scenes. The top scene updates; transitions push/replace/pop.
///
/// Hand this to [`run`](crate::run) by wrapping it in a [`Game`](crate::Game)
/// that delegates `update`/`draw`/`camera` to the stack.
pub struct SceneStack {
    stack: Vec<Box<dyn Scene>>,
    started: bool,
}

impl SceneStack {
    /// Creates a stack with `initial` as the first scene.
    pub fn new(initial: Box<dyn Scene>) -> Self {
        SceneStack {
            stack: vec![initial],
            started: false,
        }
    }

    /// Updates the top scene and applies its transition (entering new scenes
    /// immediately, so they're set up before they're drawn this frame).
    pub fn update(&mut self, world: &mut World, frame: &Frame) {
        if self.stack.is_empty() {
            return;
        }
        if !self.started {
            self.stack.last_mut().unwrap().enter(world);
            self.started = true;
        }

        let transition = self.stack.last_mut().unwrap().update(world, frame);
        match transition {
            Transition::None => {}
            Transition::Push(mut scene) => {
                scene.enter(world);
                self.stack.push(scene);
            }
            Transition::Replace(mut scene) => {
                self.stack.pop();
                scene.enter(world);
                self.stack.push(scene);
            }
            Transition::Pop => {
                self.stack.pop();
            }
        }
    }

    /// Draws the visible scenes (the top scene, plus any beneath it that show
    /// through an overlay).
    pub fn draw(&mut self, world: &World, painter: &mut Painter) {
        let n = self.stack.len();
        if n == 0 {
            return;
        }
        let mut start = n - 1;
        while start > 0 && self.stack[start].overlay() {
            start -= 1;
        }
        for scene in &mut self.stack[start..] {
            scene.draw(world, painter);
        }
    }

    /// The active scene's camera offset.
    pub fn camera(&self) -> Vec2 {
        self.stack
            .last()
            .map(|scene| scene.camera())
            .unwrap_or(Vec2::ZERO)
    }

    /// The active scene's zoom factor.
    pub fn camera_zoom(&self) -> f32 {
        self.stack
            .last()
            .map(|scene| scene.camera_zoom())
            .unwrap_or(1.0)
    }
}
