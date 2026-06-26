//! CJK-capable text rendering via ab_glyph.
//!
//! # Overview
//!
//! [`Font`] wraps an `ab_glyph` scaled font and a CPU-side glyph atlas.
//! Glyphs are rasterized on demand into an R8 coverage atlas (power-of-two,
//! initially 512×512) that is uploaded to a [`wgpu::Texture`] every frame via
//! `GlyphAtlas::flush`.  A separate per-font [`wgpu::BindGroup`] lets the 2D
//! HUD pipeline sample the atlas just like any other texture.
//!
//! The atlas uses a simple shelf-packing layout that never evicts; a font is
//! expected to render a bounded set of characters per session (the atlas is
//! rebuilt if it grows too large — see [`ATLAS_MAX_DIM`]).
//!
//! # Thread-safety
//!
//! `Font` uses `RefCell`/`Cell` for interior mutability so it can be mutated
//! from `Painter::text_font` which only receives `&mut Painter` (not `&mut
//! self`). It is **not** `Sync`.

use std::collections::HashMap;

use ab_glyph::{Font as AbFont, FontVec, PxScale, ScaleFont};

/// Maximum atlas dimension (both width and height) before the atlas is reset
/// and repacked from scratch.  WebGL2 guarantees at least 2048×2048.
const ATLAS_MAX_DIM: u32 = 2048;
/// Initial atlas side length (power of two).
const ATLAS_INIT_DIM: u32 = 512;
/// Padding between cells in the atlas (pixels), to avoid bleeding.
const CELL_PAD: u32 = 1;

/// Normalised UV rectangle inside the atlas (0.0–1.0 coords).
#[derive(Clone, Copy)]
pub(crate) struct GlyphUv {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

/// A single cached glyph: UV coords + layout metrics (all in logical pixels at
/// the size it was rasterized).
#[derive(Clone, Copy)]
pub(crate) struct CachedGlyph {
    /// UV extents inside the atlas texture.
    pub uv: GlyphUv,
    /// Width of the rendered coverage bitmap (pixels).
    pub px_w: f32,
    /// Height of the rendered coverage bitmap (pixels).
    pub px_h: f32,
    /// Left bearing from the pen position (pixels; can be negative).
    pub left: f32,
    /// Distance from the baseline to the top of the glyph bounding box (pixels; positive = above).
    pub ascent: f32,
    /// Advance width (pixels); move the pen by this much after drawing.
    pub advance: f32,
}

/// A very simple top-to-bottom shelf packer for the glyph atlas.
struct ShelfPacker {
    width: u32,
    height: u32,
    /// X cursor for the current shelf.
    shelf_x: u32,
    /// Y origin of the current shelf (top of shelf in texel coords).
    shelf_y: u32,
    /// Height of the tallest glyph in the current shelf.
    shelf_h: u32,
}

impl ShelfPacker {
    fn new(width: u32, height: u32) -> Self {
        ShelfPacker { width, height, shelf_x: 0, shelf_y: 0, shelf_h: 0 }
    }

    /// Try to allocate a `w × h` cell.  Returns `Some((x, y))` on success.
    fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        let w = w + CELL_PAD;
        let h = h + CELL_PAD;
        if self.shelf_x + w > self.width {
            // Advance to the next shelf.
            self.shelf_y += self.shelf_h;
            self.shelf_x = 0;
            self.shelf_h = 0;
        }
        if self.shelf_y + h > self.height {
            return None; // Atlas full.
        }
        let x = self.shelf_x;
        let y = self.shelf_y;
        self.shelf_x += w;
        if h > self.shelf_h {
            self.shelf_h = h;
        }
        Some((x, y))
    }
}

/// CPU-side glyph atlas: coverage bitmaps packed into a flat RGBA buffer.
///
/// The atlas texture format is `Rgba8Unorm`; glyphs are stored in the red
/// channel (R = coverage) with G/B/A = 255 so the same 2D pipeline sampler
/// works without shader changes.  The `color` tint in the draw call carries
/// the desired text colour.
pub(crate) struct GlyphAtlas {
    pub dim: u32,
    /// RGBA pixel buffer (dim × dim × 4).
    pub pixels: Vec<u8>,
    packer: ShelfPacker,
    /// Cache: (char, size_bits) -> CachedGlyph.
    cache: HashMap<(char, u32), CachedGlyph>,
    /// Whether the pixels buffer has been modified since the last `flush`.
    pub dirty: bool,
}

impl GlyphAtlas {
    pub fn new() -> Self {
        let dim = ATLAS_INIT_DIM;
        let pixels = vec![0u8; (dim * dim * 4) as usize];
        GlyphAtlas {
            dim,
            pixels,
            packer: ShelfPacker::new(dim, dim),
            cache: HashMap::new(),
            dirty: true, // initial upload needed
        }
    }

    /// Looks up a cached glyph, or rasterizes it and stores it.
    ///
    /// Returns `None` for glyphs that have no visible outline (e.g. space).
    pub fn get_or_insert(
        &mut self,
        font: &FontVec,
        ch: char,
        px_size: f32,
    ) -> Option<CachedGlyph> {
        let size_bits = px_size.to_bits();
        if let Some(g) = self.cache.get(&(ch, size_bits)) {
            return Some(*g);
        }

        let scale = PxScale::from(px_size);
        let sf = font.as_scaled(scale);
        let glyph_id = sf.glyph_id(ch);
        let advance = sf.h_advance(glyph_id);

        // Rasterize the glyph outline.
        let glyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(0.0, 0.0));
        let outlined = font.outline_glyph(glyph)?;
        let bounds = outlined.px_bounds();

        let bw = bounds.width().ceil() as u32;
        let bh = bounds.height().ceil() as u32;

        if bw == 0 || bh == 0 {
            // Zero-size glyph (e.g. space) — store a whitespace entry with no UV.
            let cached = CachedGlyph {
                uv: GlyphUv { u0: 0.0, v0: 0.0, u1: 0.0, v1: 0.0 },
                px_w: 0.0,
                px_h: 0.0,
                left: bounds.min.x,
                ascent: -bounds.min.y,
                advance,
            };
            self.cache.insert((ch, size_bits), cached);
            return Some(cached);
        }

        // Allocate space in the atlas, growing if necessary.
        let (ax, ay) = match self.packer.alloc(bw, bh) {
            Some(pos) => pos,
            None => {
                // Grow the atlas (double, up to ATLAS_MAX_DIM).
                if self.dim >= ATLAS_MAX_DIM {
                    // Atlas saturated — evict everything and restart.
                    self.dim = ATLAS_INIT_DIM;
                    self.pixels = vec![0u8; (self.dim * self.dim * 4) as usize];
                    self.packer = ShelfPacker::new(self.dim, self.dim);
                    self.cache.clear();
                } else {
                    let old_dim = self.dim;
                    let new_dim = (self.dim * 2).min(ATLAS_MAX_DIM);
                    let mut new_pixels = vec![0u8; (new_dim * new_dim * 4) as usize];
                    // Copy old rows into the top of the new buffer.
                    for row in 0..self.dim {
                        let src_start = (row * self.dim * 4) as usize;
                        let src_end = src_start + (self.dim * 4) as usize;
                        let dst_start = (row * new_dim * 4) as usize;
                        let dst_end = dst_start + (self.dim * 4) as usize;
                        new_pixels[dst_start..dst_end]
                            .copy_from_slice(&self.pixels[src_start..src_end]);
                    }
                    self.packer.width = new_dim;
                    self.packer.height = new_dim;
                    self.dim = new_dim;
                    self.pixels = new_pixels;
                    // Rescale cached UV coords from old_dim to new_dim.
                    let scale = old_dim as f32 / new_dim as f32;
                    for g in self.cache.values_mut() {
                        g.uv.u0 *= scale;
                        g.uv.v0 *= scale;
                        g.uv.u1 *= scale;
                        g.uv.v1 *= scale;
                    }
                }
                self.packer.alloc(bw, bh).expect("atlas alloc failed after grow")
            }
        };

        // Rasterize coverage into the atlas.
        // White RGB + alpha=coverage. Matches sprite.wgsl tex*tint blending.
        outlined.draw(|rx, ry, cov| {
            let px = ax + rx;
            let py = ay + ry;
            if px < self.dim && py < self.dim {
                let idx = ((py * self.dim + px) * 4) as usize;
                let alpha = (cov * 255.0 + 0.5) as u8;
                self.pixels[idx] = 255;       // R
                self.pixels[idx + 1] = 255;   // G
                self.pixels[idx + 2] = 255;   // B
                self.pixels[idx + 3] = alpha; // A = coverage
            }
        });
        self.dirty = true;

        let dim_f = self.dim as f32;
        let uv = GlyphUv {
            u0: ax as f32 / dim_f,
            v0: ay as f32 / dim_f,
            u1: (ax + bw) as f32 / dim_f,
            v1: (ay + bh) as f32 / dim_f,
        };
        let cached = CachedGlyph {
            uv,
            px_w: bw as f32,
            px_h: bh as f32,
            left: bounds.min.x,
            ascent: -bounds.min.y,
            advance,
        };
        self.cache.insert((ch, size_bits), cached);
        Some(cached)
    }
}

/// Returns the ascender height (distance from baseline to top of line, in
/// pixels) for `font` at `px_size`.  Used by `Painter::text_font` to align
/// the baseline when the caller supplies the top-left corner `y`.
pub(crate) fn font_ascent(font: &FontVec, px_size: f32) -> f32 {
    let sf = font.as_scaled(PxScale::from(px_size));
    sf.ascent()
}

/// A TTF font loaded into memory, ready for glyph rasterization.
///
/// Fonts are loaded once via [`Assets::load_font`] and can be passed to
/// [`Painter::text_font`] each frame.  The atlas is managed internally
/// and uploaded to the GPU each frame if dirty.
pub struct Font {
    pub(crate) ab: FontVec,
}

impl Font {
    /// Builds a `Font` from raw TTF/OTF bytes.
    ///
    /// Panics if the bytes are not a valid font.
    pub fn from_bytes(data: &[u8]) -> Self {
        let ab = FontVec::try_from_vec(data.to_vec())
            .expect("ab_glyph: failed to parse font bytes");
        Font { ab }
    }
}
