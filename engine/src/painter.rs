//! Immediate-mode drawing for HUD, text, and anything that isn't an entity.
//!
//! A fresh [`Painter`] is handed to [`Game::draw`](crate::Game::draw) each
//! frame; its draw calls render on top of the ECS sprites in call order.

use crate::animation::Animation;
use crate::font::{Font, GlyphAtlas, font_ascent};
use crate::math::Rect;
use crate::renderer::{DrawItem, FONT_COLS, FONT_FIRST, FONT_GLYPH_H, FONT_GLYPH_W, FONT_ROWS};
use crate::{Color, Texture};

/// Accumulates draw commands for one frame.
pub struct Painter {
    pub(crate) items: Vec<DrawItem>,
    /// Shared glyph atlas — present when the Painter was created from `app.rs`
    /// (i.e. every real frame).  `None` only in unit tests / synthetic contexts.
    atlas: Option<*mut GlyphAtlas>,
    /// Texture handle for the glyph atlas inside the renderer's texture vec.
    atlas_handle: Texture,
}

// SAFETY: `atlas` is a raw pointer to a `GlyphAtlas` that lives inside the
// `Renderer`, which lives for the duration of the frame.  `Painter` is created
// and consumed within a single frame (no cross-thread use; Octiron is
// single-threaded on WASM).
unsafe impl Send for Painter {}

impl Painter {
    /// Creates a `Painter` that can render CJK text via `text_font`.
    pub(crate) fn new_with_atlas(atlas: &mut GlyphAtlas, atlas_handle: Texture) -> Self {
        Painter {
            items: Vec::new(),
            atlas: Some(atlas as *mut GlyphAtlas),
            atlas_handle,
        }
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

    /// Draws the current frame of a grid-spritesheet [`Animation`] into `dst`.
    ///
    /// `elapsed` is the total seconds the animation has been running; the frame
    /// is selected by `anim.frame_src(elapsed)` and drawn via [`sprite_region`].
    ///
    /// [`sprite_region`]: Painter::sprite_region
    pub fn sprite_anim(&mut self, anim: &Animation, dst: Rect, elapsed: f32, color: Color) {
        let src = anim.frame_src(elapsed);
        self.sprite_region(anim.sheet, dst, src, color);
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

    /// Draws Unicode (including CJK) text using a TTF-backed [`Font`].
    ///
    /// The pen starts at `(x, y)` (top-left of the em square, i.e. the ascent
    /// line).  Each glyph is positioned using its bearing and advance so that
    /// proportional and CJK characters all look correct.
    ///
    /// `px_size` is the font size in logical pixels (e.g. `24.0`).
    ///
    /// This call is a no-op if no atlas was wired into the painter (should not
    /// happen in a normal game frame).
    pub fn text_font(&mut self, x: f32, y: f32, px_size: f32, color: Color, font: &Font, text: &str) {
        let atlas = match self.atlas {
            Some(ptr) => unsafe { &mut *ptr },
            None => return,
        };
        let atlas_handle = self.atlas_handle;

        // `y` is the top of the em-box (ascender line), consistent with
        // `Painter::text`.  The baseline sits at `y + font_ascent`.
        let baseline = y + font_ascent(&font.ab, px_size);
        let mut cursor_x = x;
        for ch in text.chars() {
            let cached = match atlas.get_or_insert(&font.ab, ch, px_size) {
                Some(g) => g,
                None => continue,
            };
            if cached.px_w > 0.0 && cached.px_h > 0.0 {
                let atlas_dim = atlas.dim as f32;
                let src = Rect::new(
                    cached.uv.u0 * atlas_dim,
                    cached.uv.v0 * atlas_dim,
                    (cached.uv.u1 - cached.uv.u0) * atlas_dim,
                    (cached.uv.v1 - cached.uv.v0) * atlas_dim,
                );
                // `cached.ascent` = -bounds.min.y = distance from baseline to
                // the top of the glyph bounding box.
                // `cached.left`   = bounds.min.x = left bearing.
                let gx = cursor_x + cached.left;
                let gy = baseline - cached.ascent;
                self.items.push(DrawItem {
                    texture: atlas_handle,
                    dst: Rect::new(gx, gy, cached.px_w, cached.px_h),
                    src: Some(src),
                    color,
                });
            }
            cursor_x += cached.advance;
        }
    }

    /// Measures the pixel width of `text` rendered with `font` at `px_size`.
    ///
    /// Sums the advance widths of all glyphs.  Returns 0.0 if the atlas is not
    /// wired (i.e. not inside a game frame).
    pub fn text_font_width(&mut self, px_size: f32, font: &Font, text: &str) -> f32 {
        let atlas = match self.atlas {
            Some(ptr) => unsafe { &mut *ptr },
            None => return 0.0,
        };
        text.chars()
            .filter_map(|ch| atlas.get_or_insert(&font.ab, ch, px_size))
            .map(|g| g.advance)
            .sum()
    }
}
