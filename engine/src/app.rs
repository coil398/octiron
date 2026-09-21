//! The winit [`ApplicationHandler`] that drives the engine: it creates the
//! window, kicks off async GPU init, runs `start` once the GPU is ready, then
//! routes input and runs update + draw + render each frame.

use std::cell::RefCell;
use std::sync::Arc;

use hecs::World;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use crate::audio::AudioCmd;
#[cfg(feature = "audio")]
use crate::audio::AudioEngine;
use crate::components::{Sprite, Transform};
use crate::input::Input;
use crate::painter::Painter;
use crate::renderer::{DrawItem, Renderer};
use crate::time::Clock;
use crate::{Color, Config, Frame, Game, Rect, Vec2};

pub(crate) struct App<G: Game> {
    game: G,
    world: World,
    input: Input,
    #[cfg(all(not(target_arch = "wasm32"), feature = "gamepad"))]
    gamepad: Option<gilrs::Gilrs>,
    clock: Clock,
    clear: Color,
    logical: Vec2,
    // Used only in the WASM async init path; intentionally dead on native.
    #[allow(dead_code)]
    proxy: EventLoopProxy<Renderer>,
    renderer: Option<Renderer>,
    window: Option<Arc<Window>>,
    initializing: bool,
    started: bool,
    /// Audio engine, lazily created on the first user-input event.
    #[cfg(feature = "audio")]
    audio: Option<AudioEngine>,
    /// Whether we have already attempted (and possibly failed) to init audio.
    #[cfg(feature = "audio")]
    audio_init_attempted: bool,
}

impl<G: Game> App<G> {
    pub(crate) fn new(game: G, config: Config, proxy: EventLoopProxy<Renderer>) -> Self {
        App {
            game,
            world: World::new(),
            input: Input::new(),
            #[cfg(all(not(target_arch = "wasm32"), feature = "gamepad"))]
            gamepad: gilrs::Gilrs::new().ok(),
            clock: Clock::new(),
            clear: config.clear_color,
            logical: config.logical_size,
            proxy,
            renderer: None,
            window: None,
            initializing: false,
            started: false,
            #[cfg(feature = "audio")]
            audio: None,
            #[cfg(feature = "audio")]
            audio_init_attempted: false,
        }
    }

    fn create_window(&self, event_loop: &ActiveEventLoop) -> Arc<Window> {
        #[allow(unused_mut)]
        let mut attributes = Window::default_attributes().with_title("Octiron");

        #[cfg(not(target_arch = "wasm32"))]
        {
            attributes = attributes.with_inner_size(winit::dpi::LogicalSize::new(
                self.logical.x as f64,
                self.logical.y as f64,
            ));
        }
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            let canvas = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id(crate::CANVAS_ID))
                .expect("missing #octiron-canvas element")
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .expect("#octiron-canvas is not a <canvas>");
            attributes = attributes.with_canvas(Some(canvas));
        }

        Arc::new(
            event_loop
                .create_window(attributes)
                .expect("failed to create window"),
        )
    }

    /// Runs `Game::start` exactly once, after the renderer (and GPU) exist.
    fn ensure_started(&mut self) {
        if self.started || self.renderer.is_none() {
            return;
        }
        let App {
            game,
            world,
            renderer,
            started,
            ..
        } = self;
        let renderer = renderer.as_mut().unwrap();
        let mut assets = crate::assets(renderer);
        game.start(world, &mut assets);
        *started = true;
    }

    /// Lazily creates the AudioEngine on the first user-input event.
    /// On browsers, AudioContext can only be created after a user gesture.
    /// When the `audio` feature is disabled this is a no-op.
    fn ensure_audio(&mut self) {
        #[cfg(feature = "audio")]
        {
            if self.audio.is_some() || self.audio_init_attempted {
                return;
            }
            self.audio_init_attempted = true;
            self.audio = AudioEngine::try_new();
        }
    }
    /// Polls the active gamepad backend and folds its state into the
    /// input as logical keys — see [`Input::sync_pad`]. The mapping
    /// follows the W3C "standard" layout: south→Space, east→Escape,
    /// west→X, north→Y, shoulders→PageUp/PageDown, select→Tab,
    /// start→Enter, d-pad and left stick→arrows.
    #[cfg(all(not(target_arch = "wasm32"), feature = "gamepad"))]
    fn poll_gamepad(&mut self) {
        let Some(gilrs) = self.gamepad.as_mut() else {
            return;
        };
        while gilrs.next_event().is_some() {}
        let mut held = std::collections::HashSet::new();
        if let Some((_id, pad)) = gilrs.gamepads().next() {
            use gilrs::{Axis, Button};
            const MAP: &[(Button, KeyCode)] = &[
                (Button::South, KeyCode::Space),
                (Button::East, KeyCode::Escape),
                (Button::West, KeyCode::KeyX),
                (Button::North, KeyCode::KeyY),
                (Button::LeftTrigger, KeyCode::PageUp),
                (Button::RightTrigger, KeyCode::PageDown),
                (Button::Select, KeyCode::Tab),
                (Button::Start, KeyCode::Enter),
                (Button::DPadUp, KeyCode::ArrowUp),
                (Button::DPadDown, KeyCode::ArrowDown),
                (Button::DPadLeft, KeyCode::ArrowLeft),
                (Button::DPadRight, KeyCode::ArrowRight),
            ];
            for (btn, key) in MAP {
                if pad.is_pressed(*btn) {
                    held.insert(*key);
                }
            }
            let x = pad.axis_data(Axis::LeftStickX).map_or(0.0, |d| d.value());
            let y = pad.axis_data(Axis::LeftStickY).map_or(0.0, |d| d.value());
            crate::input::stick_held(x, y, &mut held);
        }
        self.input.sync_pad(&held);
    }

    /// The web backend: `navigator.getGamepads()` polled once per frame
    /// under the W3C "standard" mapping — same logical keys as gilrs
    /// emits on native.
    #[cfg(target_arch = "wasm32")]
    fn poll_gamepad(&mut self) {
        use wasm_bindgen::JsCast;
        const BUTTON_MAP: &[(u32, KeyCode)] = &[
            (0, KeyCode::Space),
            (1, KeyCode::Escape),
            (2, KeyCode::KeyX),
            (3, KeyCode::KeyY),
            (4, KeyCode::PageUp),
            (5, KeyCode::PageDown),
            (8, KeyCode::Tab),
            (9, KeyCode::Enter),
            (12, KeyCode::ArrowUp),
            (13, KeyCode::ArrowDown),
            (14, KeyCode::ArrowLeft),
            (15, KeyCode::ArrowRight),
        ];
        let mut held = std::collections::HashSet::new();
        if let Some(pads) =
            web_sys::window().and_then(|w| w.navigator().get_gamepads().ok())
        {
            if let Some(pad) = pads
                .iter()
                .filter_map(|v| v.dyn_into::<web_sys::Gamepad>().ok())
                .find(|p| p.connected())
            {
                let buttons = pad.buttons();
                for (idx, key) in BUTTON_MAP {
                    let pressed = buttons
                        .get(*idx)
                        .dyn_into::<web_sys::GamepadButton>()
                        .is_ok_and(|b| b.pressed());
                    if pressed {
                        held.insert(*key);
                    }
                }
                let axes = pad.axes();
                let x = axes.get(0).as_f64().unwrap_or(0.0) as f32;
                let y = axes.get(1).as_f64().unwrap_or(0.0) as f32;
                crate::input::stick_held(x, y, &mut held);
            }
        }
        self.input.sync_pad(&held);
    }

    /// No pad backend on this build — native without `gamepad` reports
    /// an empty pad so stale holds never linger.
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "gamepad")))]
    fn poll_gamepad(&mut self) {
        self.input.sync_pad(&std::collections::HashSet::new());
    }

}

impl<G: Game> ApplicationHandler<Renderer> for App<G> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() || self.initializing {
            return;
        }
        self.initializing = true;

        let window = self.create_window(event_loop);
        self.window = Some(window.clone());
        let clear = self.clear;
        let logical = self.logical;

        #[cfg(target_arch = "wasm32")]
        {
            let proxy = self.proxy.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let renderer = Renderer::new(window, clear, logical).await;
                let _ = proxy.send_event(renderer);
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let renderer = pollster::block_on(Renderer::new(window, clear, logical));
            self.renderer = Some(renderer);
            self.initializing = false;
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, renderer: Renderer) {
        self.renderer = Some(renderer);
        self.initializing = false;
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size);
                }
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        repeat,
                        ..
                    },
                ..
            } => {
                // Lazy-init audio on first keyboard input.
                self.ensure_audio();
                match state {
                    ElementState::Pressed => self.input.press(code, repeat),
                    ElementState::Released => self.input.release(code),
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                // Convert physical cursor position to logical game coordinates,
                // inverting the letterbox transform:
                //   1. subtract the viewport offset (letterbox bars)
                //   2. divide by the uniform fit scale
                let (lx, ly) = if let Some(r) = &self.renderer {
                    let vp = r.letterbox_viewport();
                    let px = position.x as f32 - vp.x as f32;
                    let py = position.y as f32 - vp.y as f32;
                    (px / vp.scale, py / vp.scale)
                } else {
                    (position.x as f32, position.y as f32)
                };
                self.input.set_cursor(crate::Vec2::new(lx, ly));
            }

            WindowEvent::MouseInput { state, button, .. } => {
                // Lazy-init audio on first mouse input.
                self.ensure_audio();
                match state {
                    ElementState::Pressed => self.input.press_button(button),
                    ElementState::Released => self.input.release_button(button),
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                self.input.add_scroll(d);
            }

            WindowEvent::RedrawRequested => {
                if self.renderer.is_none() {
                    return;
                }
                self.ensure_started();
                self.poll_gamepad();

                let App {
                    game,
                    world,
                    input,
                    clock,
                    renderer,
                    logical,
                    #[cfg(feature = "audio")]
                    audio,
                    ..
                } = self;
                let renderer = renderer.as_mut().unwrap();

                let dt = clock.tick();
                let frame = Frame {
                    input: &*input,
                    dt,
                    screen: *logical,
                    audio: RefCell::new(Vec::new()),
                };
                game.update(world, &frame);

                // Drain the audio command queue and flush to the audio engine.
                let cmds: Vec<AudioCmd> = frame.audio.into_inner();
                #[cfg(feature = "audio")]
                if !cmds.is_empty() {
                    if let Some(eng) = audio {
                        eng.flush(cmds);
                    }
                }
                // With audio disabled, drop the queue silently.
                #[cfg(not(feature = "audio"))]
                drop(cmds);

                // World entities are drawn relative to the camera; HUD (painter)
                // stays in screen space.
                let camera = game.camera();
                let zoom = game.camera_zoom().max(0.001);
                let cx = logical.x * 0.5;
                let cy = logical.y * 0.5;
                let mut items: Vec<DrawItem> = Vec::new();
                for (transform, sprite) in world.query_mut::<(&Transform, &Sprite)>() {
                    let dst = transform.rect().offset(-camera.x, -camera.y);
                    // Apply zoom around the screen centre.
                    let dst = if (zoom - 1.0).abs() < f32::EPSILON {
                        dst
                    } else {
                        Rect::new(
                            (dst.x - cx) * zoom + cx,
                            (dst.y - cy) * zoom + cy,
                            dst.w * zoom,
                            dst.h * zoom,
                        )
                    };
                    items.push(DrawItem {
                        texture: sprite.texture,
                        dst,
                        src: sprite.src,
                        color: sprite.color,
                    });
                }

                // Submit 3D scene if the game provides one.
                if let Some(scene) = game.scene_3d() {
                    renderer.set_camera_3d(scene.camera);
                    renderer.set_texture_3d(scene.texture);
                    renderer.set_sky_3d(scene.sky);
                    for (vertices, indices) in &scene.meshes {
                        renderer.submit_mesh(vertices, indices);
                    }
                }

                let mut painter = Painter::new_with_atlas(
                    &mut renderer.glyph_atlas,
                    renderer.glyph_atlas_handle,
                );
                game.draw(&*world, &mut painter);
                items.extend(painter.items);

                renderer.render(&items);

                // "Just pressed" only lasts the frame that consumed it.
                input.end_frame();
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }
}
