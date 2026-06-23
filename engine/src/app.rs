//! The winit [`ApplicationHandler`] that drives the engine: it creates the
//! window, kicks off async GPU init, runs `start` once the GPU is ready, then
//! routes input and runs update + draw + render each frame.

use std::sync::Arc;

use hecs::World;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use crate::components::{Sprite, Transform};
use crate::input::Input;
use crate::painter::Painter;
use crate::renderer::{DrawItem, Renderer};
use crate::time::Clock;
use crate::{Color, Config, Frame, Game, Vec2};

pub(crate) struct App<G: Game> {
    game: G,
    world: World,
    input: Input,
    clock: Clock,
    clear: Color,
    logical: Vec2,
    proxy: EventLoopProxy<Renderer>,
    renderer: Option<Renderer>,
    window: Option<Arc<Window>>,
    initializing: bool,
    started: bool,
}

impl<G: Game> App<G> {
    pub(crate) fn new(game: G, config: Config, proxy: EventLoopProxy<Renderer>) -> Self {
        App {
            game,
            world: World::new(),
            input: Input::new(),
            clock: Clock::new(),
            clear: config.clear_color,
            logical: config.logical_size,
            proxy,
            renderer: None,
            window: None,
            initializing: false,
            started: false,
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
            } => match state {
                ElementState::Pressed => self.input.press(code, repeat),
                ElementState::Released => self.input.release(code),
            },

            WindowEvent::RedrawRequested => {
                if self.renderer.is_none() {
                    return;
                }
                self.ensure_started();

                let App {
                    game,
                    world,
                    input,
                    clock,
                    renderer,
                    logical,
                    ..
                } = self;
                let renderer = renderer.as_mut().unwrap();

                let dt = clock.tick();
                let frame = Frame {
                    input: &*input,
                    dt,
                    screen: *logical,
                };
                game.update(world, &frame);

                // World entities are drawn relative to the camera; HUD (painter)
                // stays in screen space.
                let camera = game.camera();
                let mut items: Vec<DrawItem> = Vec::new();
                for (transform, sprite) in world.query_mut::<(&Transform, &Sprite)>() {
                    items.push(DrawItem {
                        texture: sprite.texture,
                        dst: transform.rect().offset(-camera.x, -camera.y),
                        src: sprite.src,
                        color: sprite.color,
                    });
                }

                let mut painter = Painter::new();
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
