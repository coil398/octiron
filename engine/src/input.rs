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
    // gamepad: logical keys the pad held last frame (diffed per frame)
    pad_down: HashSet<KeyCode>,
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

    /// Reconciles the gamepad's held logical keys against last frame's:
    /// newly held emit `press`, newly released emit `release` — the same
    /// events the keyboard produces, so a game reads one vocabulary and
    /// never sees the pad. Called once per frame by the active backend.
    ///
    /// Both devices share the `KeyCode` space: a pad release clears a
    /// key the keyboard is also holding — pad and keyboard playing the
    /// same key at once is undefined by design.
    pub(crate) fn sync_pad(&mut self, held: &HashSet<KeyCode>) {
        let new: Vec<KeyCode> = held.difference(&self.pad_down).copied().collect();
        let gone: Vec<KeyCode> = self.pad_down.difference(held).copied().collect();
        for key in new {
            self.press(key, false);
        }
        for key in gone {
            self.release(key);
        }
        self.pad_down.clone_from(held);
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

/// Left-stick deflection that counts as a direction; below it the
/// stick reads as at rest.
#[cfg(any(target_arch = "wasm32", feature = "gamepad", test))]
pub(crate) const STICK_THRESHOLD: f32 = 0.5;

/// Registers left-stick deflection as arrow-key holds in `held` — the
/// shared half of every backend's pad translation.
#[cfg(any(target_arch = "wasm32", feature = "gamepad", test))]
pub(crate) fn stick_held(x: f32, y: f32, held: &mut HashSet<KeyCode>) {
    if x <= -STICK_THRESHOLD {
        held.insert(KeyCode::ArrowLeft);
    }
    if x >= STICK_THRESHOLD {
        held.insert(KeyCode::ArrowRight);
    }
    if y <= -STICK_THRESHOLD {
        held.insert(KeyCode::ArrowUp);
    }
    if y >= STICK_THRESHOLD {
        held.insert(KeyCode::ArrowDown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_press_emits_the_same_key_event_as_the_keyboard() {
        let mut input = Input::new();
        let held: HashSet<_> = [KeyCode::ArrowLeft, KeyCode::Space].into_iter().collect();
        input.sync_pad(&held);
        assert!(input.is_key_down(KeyCode::ArrowLeft));
        assert!(input.is_key_pressed(KeyCode::ArrowLeft));
        assert!(input.is_key_pressed(KeyCode::Space));
    }

    #[test]
    fn a_held_pad_key_stays_down_without_repeating() {
        let mut input = Input::new();
        let held: HashSet<_> = [KeyCode::ArrowLeft].into_iter().collect();
        input.sync_pad(&held);
        input.end_frame();
        input.sync_pad(&held);
        assert!(input.is_key_down(KeyCode::ArrowLeft));
        assert!(!input.is_key_pressed(KeyCode::ArrowLeft), "no re-press");
    }

    #[test]
    fn a_released_pad_key_emits_release() {
        let mut input = Input::new();
        let held: HashSet<_> = [KeyCode::ArrowLeft].into_iter().collect();
        input.sync_pad(&held);
        input.end_frame();
        input.sync_pad(&HashSet::new());
        assert!(!input.is_key_down(KeyCode::ArrowLeft));
    }

    #[test]
    fn the_stick_rest_zone_holds_no_direction() {
        let mut held = HashSet::new();
        stick_held(0.4, -0.4, &mut held);
        assert!(held.is_empty());
        stick_held(-1.0, 0.0, &mut held);
        assert!(held.contains(&KeyCode::ArrowLeft));
    }
}
