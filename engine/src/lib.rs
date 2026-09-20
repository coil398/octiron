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

mod animation;
mod app;
mod audio;
mod autotile;
mod components;
pub(crate) mod font;
mod input;
mod math;
pub mod math3d;
mod painter;
mod renderer;
mod scene;
mod storage;
mod time;

pub use animation::Animation;
pub use audio::Sound;
pub use autotile::autotile_index_4bit;
pub use components::{Sprite, Transform};
pub use font::Font;
pub use renderer::FONT_GLYPH_H;
pub use input::Input;
pub use math::{Rect, Vec2};
pub use math3d::{CameraUniform, Vertex3D, cube_mesh, diorama_view, proj_ortho, proj_persp};
pub use painter::Painter;
pub use storage::Storage;
pub use scene::{Scene, SceneStack, Transition};

/// Re-exported ECS types from [hecs]. Define your own component types freely;
/// hecs requires no registration.
pub use hecs::{Entity, World};

/// Physical key identifiers, re-exported from winit (e.g. `Key::ArrowLeft`,
/// `Key::KeyW`, `Key::Space`).
pub use winit::keyboard::KeyCode as Key;

/// Mouse button identifiers, re-exported from winit (e.g. `MouseButton::Left`,
/// `MouseButton::Right`).
pub use winit::event::MouseButton;

use std::cell::RefCell;

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

    /// Decodes audio bytes (WAV or MP3; e.g. `include_bytes!("assets/sound.wav")`)
    /// and returns a [`Sound`] handle for use with [`Frame::play_sound`] and
    /// [`Frame::play_music`].
    ///
    /// Returns `None` if the bytes cannot be decoded (unsupported format, corrupt
    /// data, etc.).  Uses `from_cursor` internally so it works on both native and
    /// WASM targets.
    pub fn load_sound(&mut self, bytes: &'static [u8]) -> Option<Sound> {
        audio::load_sound_bytes(bytes)
    }

    /// Builds a [`Font`] from raw TTF/OTF bytes (e.g.
    /// `include_bytes!("assets/NotoSansCJK.ttf")`).
    ///
    /// The returned [`Font`] can be passed to [`Painter::text_font`] to render
    /// Unicode / CJK text on top of the scene each frame.  The glyph atlas is
    /// shared across all fonts and lazily populated on first use.
    ///
    /// Existing games that do not call this method continue using the built-in
    /// ASCII bitmap font via [`Painter::text`], which is unaffected.
    pub fn load_font(&mut self, ttf_bytes: &[u8]) -> Font {
        Font::from_bytes(ttf_bytes)
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
    /// Internal audio command queue (interior mutability so `update` takes `&Frame`).
    pub(crate) audio: RefCell<Vec<audio::AudioCmd>>,
}

impl<'a> Frame<'a> {
    /// Plays `sound` once (one-shot / SFX).
    ///
    /// The request is queued and executed after [`Game::update`] returns.
    /// If the audio engine has not been initialised yet (before the first user
    /// gesture on the web), the request is silently dropped.
    pub fn play_sound(&self, sound: Sound) {
        self.audio.borrow_mut().push(audio::AudioCmd::PlaySound(sound.0));
    }

    /// Starts `sound` as a looping background music track.
    ///
    /// Any previously playing BGM is stopped.  The request is queued and executed
    /// after [`Game::update`] returns.
    pub fn play_music(&self, sound: Sound) {
        self.audio.borrow_mut().push(audio::AudioCmd::PlayMusic(sound.0));
    }

    /// Sets the master output volume.  `volume` is clamped to `0.0..=1.0`.
    pub fn set_master_volume(&self, volume: f32) {
        self.audio.borrow_mut().push(audio::AudioCmd::SetMasterVolume(volume));
    }
}

/// A 3D scene submitted by a game for one frame.
///
/// Call [`renderer::Renderer::submit_mesh`] + [`renderer::Renderer::set_camera_3d`]
/// by returning this from [`Game::scene_3d`].  Returning `None` skips the 3D pass.
pub struct Scene3D {
    /// Camera and lighting parameters.
    pub camera: CameraUniform,
    /// Meshes: each entry is `(vertices, indices)` with model-space geometry.
    /// The engine appends them into a shared frame buffer and draws in order.
    pub meshes: Vec<(Vec<Vertex3D>, Vec<u32>)>,
    /// Optional texture to sample in the 3D fragment shader.
    ///
    /// When `None` (the default), the engine binds a 1×1 white texture so that
    /// `tex(uv) = [1,1,1,1]` and the output equals the old vertex-color ×
    /// directional + ambient lighting pipeline.  All existing games (diorama-demo,
    /// continent-sim Solid3D, etc.) leave this `None` and are pixel-identical.
    ///
    /// To use a texture, load it via [`Assets::load_png`] in [`Game::start`] and
    /// store the returned [`Texture`] handle, then supply it here each frame.
    /// UV coordinates come from [`Vertex3D::uv`].
    pub texture: Option<Texture>,
    /// Background color used to clear the 3D color attachment at the start of the
    /// 3D pass.  When `None` (the default), the engine clears to the [`Config::clear_color`]
    /// set at startup — identical to the previous behaviour.
    ///
    /// Supply an RGBA value (components in `0.0..=1.0`) to override the sky color
    /// on a per-frame basis, e.g. `sky: Some([0.4, 0.6, 0.9, 1.0])`.
    pub sky: Option<[f32; 4]>,
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

    /// Returns 3D scene data for this frame, or `None` to skip the 3D pass.
    /// Existing 2D games simply leave this unimplemented (default = `None`).
    fn scene_3d(&self) -> Option<Scene3D> {
        None
    }

    /// World-space camera offset (pixels) applied to entity rendering. Sprites
    /// are drawn at `position - camera`; [`Painter`] HUD is unaffected. Default
    /// `Vec2::ZERO` (no scrolling).
    fn camera(&self) -> Vec2 {
        Vec2::ZERO
    }

    /// Uniform zoom factor applied to entity rendering, scaling the scene around
    /// the screen centre. `1.0` = no zoom (default). Values > 1.0 zoom in;
    /// values 0 < z < 1.0 zoom out. [`Painter`] HUD is unaffected.
    fn camera_zoom(&self) -> f32 {
        1.0
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
