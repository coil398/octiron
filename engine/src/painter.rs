//! Immediate-mode drawing for HUD, text, and anything that isn't an entity.
//!
//! A fresh [`Painter`] is handed to [`Game::draw`](crate::Game::draw) each
//! frame; its draw calls render on top of the ECS sprites in call order.

use crate::math::Rect;
use crate::renderer::{DrawItem, FONT_COLS, FONT_FIRST, FONT_GLYPH_H, FONT_GLYPH_W, FONT_ROWS};
use crate::{Color, Texture};

/// Accumulates draw commands for one frame.
pub struct Painter {
    pub(crate) items: Vec<DrawItem>,
}

impl Painter {
    pub(crate) fn new() -> Self {
        Painter { items: Vec::new() }
    }

    /// Draws a solid-colored rectangle.
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        self.items.push(DrawItem {
            texture: Texture::WHITE,
            dst: Rect::new(x, y, w, h),
            src: None,
            color,
        });
    }

    /// Draws `texture` (whole image) into the destination rect, tinted.
    pub fn sprite(&mut self, texture: Texture, x: f32, y: f32, w: f32, h: f32, color: Color) {
        self.items.push(DrawItem {
            texture,
            dst: Rect::new(x, y, w, h),
            src: None,
            color,
        });
    }

    /// Draws a sub-rect (`src`, in texels) of `texture` into the destination.
    pub fn sprite_region(&mut self, texture: Texture, dst: Rect, src: Rect, color: Color) {
        self.items.push(DrawItem {
            texture,
            dst,
            src: Some(src),
            color,
        });
    }

    /// Advance width (pixels) of one glyph at the given scale.
    pub fn glyph_advance(scale: f32) -> f32 {
        FONT_GLYPH_W * scale
    }

    /// Pixel width of `text` at the given scale.
    pub fn text_width(&self, text: &str, scale: f32) -> f32 {
        text.chars().count() as f32 * Painter::glyph_advance(scale)
    }

    /// Draws monospace text with its top-left at `(x, y)`, tinted by `color`.
    /// Only ASCII is rendered; other chars advance as blanks.
    pub fn text(&mut self, x: f32, y: f32, scale: f32, color: Color, text: &str) {
        let gw = FONT_GLYPH_W * scale;
        let gh = FONT_GLYPH_H * scale;
        let mut cursor = x;
        for ch in text.chars() {
            let code = ch as u32;
            if code >= FONT_FIRST && code < FONT_FIRST + FONT_COLS * FONT_ROWS {
                let index = code - FONT_FIRST;
                let col = index % FONT_COLS;
                let row = index / FONT_COLS;
                let src = Rect::new(
                    col as f32 * FONT_GLYPH_W,
                    row as f32 * FONT_GLYPH_H,
                    FONT_GLYPH_W,
                    FONT_GLYPH_H,
                );
                self.items.push(DrawItem {
                    texture: Texture::FONT,
                    dst: Rect::new(cursor, y, gw, gh),
                    src: Some(src),
                    color,
                });
            }
            cursor += gw;
        }
    }

    /// Draws text centered horizontally on `center_x`.
    pub fn text_centered(&mut self, center_x: f32, y: f32, scale: f32, color: Color, text: &str) {
        let w = self.text_width(text, scale);
        self.text(center_x - w * 0.5, y, scale, color, text);
    }
}
