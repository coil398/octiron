//! Keyboard and mouse input state, updated from winit events.

use std::collections::HashSet;

use winit::event::MouseButton;
use winit::keyboard::KeyCode;

use crate::math::Vec2;

/// Tracks which physical keys are held, and which were first pressed this frame.
///
/// Keys are identified by winit's [`KeyCode`] (re-exported from the crate root
/// as `octiron::Key`), e.g. `Key::ArrowLeft`, `Key::KeyW`, `Key::Space`.
///
/// Mouse state is also tracked here: cursor position in logical pixels,
/// button down/just-pressed/just-released state, and per-frame scroll delta.
#[derive(Default)]
pub struct Input {
    // keyboard
    down: HashSet<KeyCode>,
    pressed: HashSet<KeyCode>,
    // mouse cursor
    cursor: Vec2,
    // mouse buttons
    btn_down: HashSet<MouseButton>,
    btn_pressed: HashSet<MouseButton>,
    btn_released: HashSet<MouseButton>,
    // per-frame scroll accumulator
    scroll: f32,
}

impl Input {
    pub(crate) fn new() -> Self {
        Input::default()
    }

    // ---- keyboard -----------------------------------------------------------

    /// `repeat` is winit's auto-repeat flag; a repeat is held, not newly pressed.
    pub(crate) fn press(&mut self, key: KeyCode, repeat: bool) {
        self.down.insert(key);
        if !repeat {
            self.pressed.insert(key);
        }
    }

    pub(crate) fn release(&mut self, key: KeyCode) {
        self.down.remove(&key);
    }

    // ---- mouse setters (called by app.rs) -----------------------------------

    /// Updates cursor position in logical pixel coordinates.
    pub(crate) fn set_cursor(&mut self, p: Vec2) {
        self.cursor = p;
    }

    /// Records a mouse button press for this frame.
    pub(crate) fn press_button(&mut self, b: MouseButton) {
        self.btn_down.insert(b);
        self.btn_pressed.insert(b);
    }

    /// Records a mouse button release for this frame.
    pub(crate) fn release_button(&mut self, b: MouseButton) {
        self.btn_down.remove(&b);
        self.btn_released.insert(b);
    }

    /// Accumulates scroll delta for this frame.
    pub(crate) fn add_scroll(&mut self, d: f32) {
        self.scroll += d;
    }

    // ---- frame boundary -----------------------------------------------------

    /// Clears all per-frame sets; called once per frame.
    pub(crate) fn end_frame(&mut self) {
        self.pressed.clear();
        self.btn_pressed.clear();
        self.btn_released.clear();
        self.scroll = 0.0;
    }

    // ---- keyboard getters ---------------------------------------------------

    /// Returns `true` while `key` is held down.
    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.down.contains(&key)
    }

    /// Returns `true` only on the frame `key` was first pressed (no auto-repeat).
    /// Use this for one-shot actions like a flap or a jump.
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }

    // ---- mouse getters ------------------------------------------------------

    /// Current cursor position in logical pixel coordinates.
    pub fn cursor(&self) -> Vec2 {
        self.cursor
    }

    /// Returns `true` while the mouse button is held down.
    pub fn is_mouse_down(&self, b: MouseButton) -> bool {
        self.btn_down.contains(&b)
    }

    /// Returns `true` only on the frame the mouse button was first pressed.
    pub fn is_mouse_pressed(&self, b: MouseButton) -> bool {
        self.btn_pressed.contains(&b)
    }

    /// Returns `true` only on the frame the mouse button was released.
    pub fn is_mouse_released(&self, b: MouseButton) -> bool {
        self.btn_released.contains(&b)
    }

    /// Accumulated mouse-wheel delta this frame (positive = scroll up / away).
    pub fn scroll(&self) -> f32 {
        self.scroll
    }
}
