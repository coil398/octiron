//! Keyboard input state, updated from winit keyboard events.

use std::collections::HashSet;

use winit::keyboard::KeyCode;

/// Tracks which physical keys are held, and which were first pressed this frame.
///
/// Keys are identified by winit's [`KeyCode`] (re-exported from the crate root
/// as `octiron::Key`), e.g. `Key::ArrowLeft`, `Key::KeyW`, `Key::Space`.
#[derive(Default)]
pub struct Input {
    down: HashSet<KeyCode>,
    pressed: HashSet<KeyCode>,
}

impl Input {
    pub(crate) fn new() -> Self {
        Input::default()
    }

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

    /// Clears the per-frame "just pressed" set; called once per frame.
    pub(crate) fn end_frame(&mut self) {
        self.pressed.clear();
    }

    /// Returns `true` while `key` is held down.
    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.down.contains(&key)
    }

    /// Returns `true` only on the frame `key` was first pressed (no auto-repeat).
    /// Use this for one-shot actions like a flap or a jump.
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }
}
