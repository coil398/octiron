//! Built-in components the engine understands.
//!
//! Any entity carrying both a [`Transform`] and a [`Sprite`] is drawn
//! automatically. Games may attach their own components alongside these
//! (hecs requires no registration).

use crate::math::{Rect, Vec2};
use crate::{Color, Texture};

/// Position and size of an entity in screen pixels (origin top-left, y-down).
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    /// Top-left corner, in pixels.
    pub position: Vec2,
    /// Width and height, in pixels.
    pub size: Vec2,
}

impl Transform {
    pub fn new(position: Vec2, size: Vec2) -> Self {
        Transform { position, size }
    }

    /// This transform as an axis-aligned [`Rect`], handy for collision tests.
    pub fn rect(&self) -> Rect {
        Rect::new(self.position.x, self.position.y, self.size.x, self.size.y)
    }
}

/// How an entity's [`Transform`] rectangle is filled: a texture region tinted
/// by a color. A solid color is just the built-in white texture tinted.
#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    /// Texture to sample (defaults to the built-in white texture).
    pub texture: Texture,
    /// Source sub-rect in texels (`None` = whole texture).
    pub src: Option<Rect>,
    /// Tint multiplied with the sampled texel (white = unchanged).
    pub color: Color,
}

impl Sprite {
    /// A solid-colored fill.
    pub fn color(color: Color) -> Self {
        Sprite {
            texture: Texture::WHITE,
            src: None,
            color,
        }
    }

    /// A full-texture sprite, untinted.
    pub fn texture(texture: Texture) -> Self {
        Sprite {
            texture,
            src: None,
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }

    /// A sub-rect (in texels) of a texture, untinted.
    pub fn region(texture: Texture, src: Rect) -> Self {
        Sprite {
            texture,
            src: Some(src),
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }

    /// Sets the tint color (builder style).
    pub fn tinted(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}
