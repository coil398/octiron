//! Autotile helpers for tilemap rendering.
//!
//! ## 4-neighbour blob autotiling
//!
//! [`autotile_index_4bit`] maps a 4-bit neighbour mask to a tile index in the
//! standard **16-tile blob layout** (a 4×4 tileset, row-major):
//!
//! ```text
//! index = row * 4 + col
//!
//!      col 0   col 1   col 2   col 3
//! row 0:  0       1       2       3      (mask 0b0000 .. 0b0011)
//! row 1:  4       5       6       7      (mask 0b0100 .. 0b0111)
//! row 2:  8       9      10      11      (mask 0b1000 .. 0b1011)
//! row 3: 12      13      14      15      (mask 0b1100 .. 0b1111)
//! ```
//!
//! The 4-bit mask encodes which of the four cardinal neighbours is the **same
//! tile type** as the centre tile:
//!
//! | bit | direction |
//! |-----|-----------|
//! |  0  | up        |
//! |  1  | right     |
//! |  2  | down      |
//! |  3  | left      |
//!
//! A bit is **set (1)** when the corresponding neighbour exists and is the same
//! type; **clear (0)** when absent or a different type.
//!
//! Because the index is simply the mask value itself (`mask as usize`), the
//! mapping is trivial—but this function makes the convention explicit and
//! self-documenting.
//!
//! ### Example
//!
//! ```
//! use octiron::autotile_index_4bit;
//!
//! // Isolated tile: no same-type neighbours → index 0 (top-left of the sheet)
//! assert_eq!(autotile_index_4bit(0b0000), 0);
//!
//! // Connected left and right only → index 10 (0b1010)
//! assert_eq!(autotile_index_4bit(0b1010), 10);
//!
//! // All four neighbours present → index 15 (fully interior tile)
//! assert_eq!(autotile_index_4bit(0b1111), 15);
//! ```

/// Maps a 4-neighbour bitmask to a tile index in the standard 16-tile blob
/// layout (row-major 4×4 sheet).
///
/// # Parameters
///
/// - `mask`: a 4-bit value whose bits encode same-type neighbours:
///   - bit 0 (LSB) — **up** neighbour present
///   - bit 1       — **right** neighbour present
///   - bit 2       — **down** neighbour present
///   - bit 3       — **left** neighbour present
///
///   Only the lower 4 bits are used; the upper bits are ignored.
///
/// # Returns
///
/// An index in `0..=15`.  The index equals `mask & 0xF`, which directly
/// encodes the row-major position in the 4×4 tileset:
/// `index = row * 4 + col` where `row = (mask >> 2) & 3` and
/// `col = mask & 3`.
///
/// # Example
///
/// ```
/// use octiron::autotile_index_4bit;
///
/// assert_eq!(autotile_index_4bit(0b0000), 0);   // isolated
/// assert_eq!(autotile_index_4bit(0b0001), 1);   // up only
/// assert_eq!(autotile_index_4bit(0b1111), 15);  // all neighbours
/// ```
pub fn autotile_index_4bit(mask: u8) -> usize {
    (mask & 0xF) as usize
}

#[cfg(test)]
mod tests {
    use super::autotile_index_4bit;

    #[test]
    fn isolated_tile_is_index_0() {
        assert_eq!(autotile_index_4bit(0b0000), 0);
    }

    #[test]
    fn single_neighbours() {
        assert_eq!(autotile_index_4bit(0b0001), 1);  // up
        assert_eq!(autotile_index_4bit(0b0010), 2);  // right
        assert_eq!(autotile_index_4bit(0b0100), 4);  // down
        assert_eq!(autotile_index_4bit(0b1000), 8);  // left
    }

    #[test]
    fn fully_connected_is_index_15() {
        assert_eq!(autotile_index_4bit(0b1111), 15);
    }

    #[test]
    fn horizontal_corridor() {
        // left + right connected (bits 1 and 3)
        assert_eq!(autotile_index_4bit(0b1010), 10);
    }

    #[test]
    fn vertical_corridor() {
        // up + down connected (bits 0 and 2)
        assert_eq!(autotile_index_4bit(0b0101), 5);
    }

    #[test]
    fn upper_bits_ignored() {
        // 0b1111_1111 should give the same result as 0b0000_1111
        assert_eq!(autotile_index_4bit(0xFF), 15);
        assert_eq!(autotile_index_4bit(0b1111_0000), 0);
    }

    #[test]
    fn all_16_indices_covered() {
        for mask in 0u8..=15 {
            let idx = autotile_index_4bit(mask);
            assert!(idx < 16, "index {idx} out of range for mask {mask:#06b}");
        }
    }
}
