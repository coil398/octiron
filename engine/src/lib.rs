//! # Octiron
//!
//! A small, from-scratch 2D game engine for the web and native desktop. Games
//! are written in Rust, organized with an ECS ([hecs]), rendered with [wgpu]
//! (textured sprites + a built-in bitmap font), and—on the web—compiled to
//! WebAssembly.
//!
//! A game implements [`Game`]:
//! - [`start`](Game::start) spawns entities and loads textures via [`Assets`];
//! - [`update`](Game::update) advances the simulation each frame;
//! - [`draw`](Game::draw) (optional) paints HUD/text over the entities.
//!
//! Any entity with a [`Transform`] and a [`Sprite`] is drawn automatically.
//!
//! ```no_run
//! use octiron::{run, Assets, Frame, Game, Painter, Sprite, Transform, Vec2, World};
//!
//! struct MyGame;
//! impl Game for MyGame {
//!     fn start(&mut self, world: &mut World, _assets: &mut Assets) {
//!         world.spawn((
//!             Transform::new(Vec2::new(100.0, 100.0), Vec2::new(80.0, 80.0)),
//!             Sprite::color([0.3, 0.8, 1.0, 1.0]),
//!         ));
//!     }
//!     fn update(&mut self, _world: &mut World, _frame: &Frame) {}
//!     fn draw(&mut self, _world: &World, painter: &mut Painter) {
//!         painter.text(16.0, 16.0, 0.5, [1.0; 4], "HELLO");
//!     }
//! }
//!
//! # fn entry() {
//! run(MyGame);
//! # }
//! ```

mod app;
mod components;
mod input;
mod math;
mod painter;
mod renderer;
mod scene;
mod time;

pub use components::{Sprite, Transform};
pub use input::Input;
pub use math::{Rect, Vec2};
pub use painter::Painter;
pub use scene::{Scene, SceneStack, Transition};

/// Re-exported ECS types from [hecs]. Define your own component types freely;
/// hecs requires no registration.
pub use hecs::{Entity, World};

/// Physical key identifiers, re-exported from winit (e.g. `Key::ArrowLeft`,
/// `Key::KeyW`, `Key::Space`).
pub use winit::keyboard::KeyCode as Key;

use winit::event_loop::EventLoop;

/// An RGBA color with components in `0.0..=1.0`.
pub type Color = [f32; 4];

/// The id of the `<canvas>` element the engine renders into on the web.
pub const CANVAS_ID: &str = "octiron-canvas";

/// A handle to a loaded texture. `WHITE` and `FONT` are always available.
/// The default is [`Texture::WHITE`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Texture(pub(crate) u32);

impl Texture {
    /// A built-in 1x1 white texture (solid fills are this, tinted).
    pub const WHITE: Texture = Texture(0);
    /// The built-in monospace font atlas (used by [`Painter::text`]).
    pub const FONT: Texture = Texture(1);
}

/// A tiny deterministic LCG random generator, so games don't need an rng crate.
///
/// ```
/// let mut rng = octiron::Rng::new(0x1234_5678);
/// let x = rng.range(0.0, 800.0);
/// ```
pub struct Rng(u32);

impl Rng {
    /// Creates a generator from a seed.
    pub fn new(seed: u32) -> Self {
        Rng(seed)
    }

    /// Returns the next raw `u32`.
    pub fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        self.0
    }

    /// Returns the next `f32` in `0.0..1.0`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Returns an `f32` in `lo..hi`.
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    /// Returns an integer in `0..n` (treats `n <= 0` as `1`).
    pub fn below(&mut self, n: i32) -> i32 {
        (self.next_u32() >> 8) as i32 % n.max(1)
    }
}

/// Loads textures during [`Game::start`].
///
/// `Assets` is only available inside [`Game::start`] (it borrows the renderer),
/// so load every texture there—not in `update`.
pub struct Assets<'a> {
    renderer: &'a mut renderer::Renderer,
}

impl<'a> Assets<'a> {
    /// Decodes image bytes (PNG; e.g. `include_bytes!("assets/ship.png")`) and
    /// uploads them, returning a handle usable in [`Sprite`] and [`Painter`].
    pub fn load_png(&mut self, png_bytes: &[u8]) -> Texture {
        self.renderer.load_texture(png_bytes)
    }

    /// The pixel dimensions of a loaded texture.
    pub fn texture_size(&self, texture: Texture) -> Vec2 {
        self.renderer.texture_size(texture)
    }
}

/// Per-frame state handed to [`Game::update`].
pub struct Frame<'a> {
    /// Current keyboard state.
    pub input: &'a Input,
    /// Seconds elapsed since the previous frame (clamped against long stalls).
    pub dt: f32,
    /// The logical drawable size in pixels.
    pub screen: Vec2,
}

/// A game driven by the engine.
pub trait Game: 'static {
    /// Spawns initial entities and loads textures. Runs once, after the GPU is
    /// ready.
    fn start(&mut self, world: &mut World, assets: &mut Assets);

    /// Advances the game by one frame.
    fn update(&mut self, world: &mut World, frame: &Frame);

    /// Paints HUD/text over the entities. Optional; default draws nothing.
    fn draw(&mut self, world: &World, painter: &mut Painter) {
        let _ = (world, painter);
    }

    /// World-space camera offset (pixels) applied to entity rendering. Sprites
    /// are drawn at `position - camera`; [`Painter`] HUD is unaffected. Default
    /// `Vec2::ZERO` (no scrolling).
    fn camera(&self) -> Vec2 {
        Vec2::ZERO
    }
}

/// Engine startup options.
pub struct Config {
    /// Background color the screen is cleared to each frame.
    pub clear_color: Color,
    /// Fixed logical resolution games draw in (stretched to fill the canvas).
    pub logical_size: Vec2,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            clear_color: [0.07, 0.08, 0.12, 1.0],
            logical_size: Vec2::new(800.0, 600.0),
        }
    }
}

/// Boots the engine with default [`Config`] and runs `game` forever.
pub fn run<G: Game>(game: G) {
    run_with(game, Config::default());
}

/// Like [`run`], but with explicit [`Config`].
pub fn run_with<G: Game>(game: G, config: Config) {
    init_platform();

    let event_loop = EventLoop::<renderer::Renderer>::with_user_event()
        .build()
        .expect("failed to build event loop");

    let proxy = event_loop.create_proxy();
    let app = app::App::new(game, config, proxy);

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(app);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = app;
        event_loop.run_app(&mut app).expect("event loop error");
    }
}

#[cfg(target_arch = "wasm32")]
fn init_platform() {
    console_error_panic_hook::set_once();
}

#[cfg(not(target_arch = "wasm32"))]
fn init_platform() {
    let _ = env_logger::try_init();
}

/// Constructs an [`Assets`] view over the renderer (used by the app on start).
pub(crate) fn assets(renderer: &mut renderer::Renderer) -> Assets<'_> {
    Assets { renderer }
}
