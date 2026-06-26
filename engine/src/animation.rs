//! Sprite-sheet animation helper.
//!
//! An [`Animation`] describes a grid spritesheet where every frame cell has the
//! same pixel dimensions.  Call [`Animation::frame_src`] with the elapsed
//! seconds to get the source [`Rect`] for the current looping frame, then draw
//! it with [`crate::Painter::sprite_anim`] or [`crate::Painter::sprite_region`].

use crate::math::Rect;
use crate::Texture;

/// A looping grid-spritesheet animation.
///
/// # Layout
///
/// The sheet is divided into rows and columns of equal-sized cells.  Frame
/// indices are ordered left-to-right, top-to-bottom (row-major).
///
/// # Example
///
/// ```no_run
/// use octiron::{Animation, Texture};
///
/// // 8-frame run cycle on a 512×64 sheet (single row, 64×64 cells, 12 fps).
/// let walk = Animation {
///     sheet:   Texture::WHITE, // replace with your loaded texture
///     frame_w: 64.0,
///     frame_h: 64.0,
///     frames:  8,
///     columns: 8,
///     fps:     12.0,
/// };
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Animation {
    /// The spritesheet texture.
    pub sheet: Texture,
    /// Width of one frame cell in texels.
    pub frame_w: f32,
    /// Height of one frame cell in texels.
    pub frame_h: f32,
    /// Total number of frames in the animation (≥ 1).
    pub frames: u32,
    /// Number of columns in the spritesheet grid (≥ 1).
    pub columns: u32,
    /// Playback speed in frames per second.
    pub fps: f32,
}

impl Animation {
    /// Returns the source [`Rect`] (in texels) for the frame that should be
    /// displayed at `elapsed_secs` seconds into the animation.
    ///
    /// The animation loops continuously.  If `frames` or `columns` is zero the
    /// method behaves as if they are 1 to avoid a divide-by-zero.
    pub fn frame_src(&self, elapsed_secs: f32) -> Rect {
        let idx = ((elapsed_secs * self.fps) as u32) % self.frames.max(1);
        let col = idx % self.columns.max(1);
        let row = idx / self.columns.max(1);
        Rect::new(
            col as f32 * self.frame_w,
            row as f32 * self.frame_h,
            self.frame_w,
            self.frame_h,
        )
    }
}
