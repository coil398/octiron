use crate::*;
use octiron::{CameraUniform, Scene3D, Vertex3D, autotile_index_4bit, diorama_view, proj_ortho};
use glam::Vec3;

// ---- overlay / art-style enums --------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Overlay {
    Terrain,
    Carrying,
    Population,
    Nation,
    Tech,
    Resource,
}

impl Overlay {
    pub(crate) fn next(self) -> Overlay {
        match self {
            Overlay::Terrain   => Overlay::Carrying,
            Overlay::Carrying  => Overlay::Population,
            Overlay::Population => Overlay::Nation,
            Overlay::Nation    => Overlay::Tech,
            Overlay::Tech      => Overlay::Resource,
            Overlay::Resource  => Overlay::Terrain,
        }
    }

    pub(crate) fn label_jp(self) -> &'static str {
        match self {
            Overlay::Terrain    => "地形",
            Overlay::Carrying   => "収容力",
            Overlay::Population => "人口",
            Overlay::Nation     => "国家",
            Overlay::Tech       => "技術",
            Overlay::Resource   => "資源",
        }
    }
}

/// Which generated art set the map is rendered with. Toggled with [G].
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtStyle {
    Pixel,
    Diorama,
    Solid3D,
}

impl ArtStyle {
    pub(crate) fn next(self) -> ArtStyle {
        match self {
            ArtStyle::Pixel    => ArtStyle::Diorama,
            ArtStyle::Diorama  => ArtStyle::Solid3D,
            ArtStyle::Solid3D  => ArtStyle::Pixel,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            ArtStyle::Pixel    => "PIXEL",
            ArtStyle::Diorama  => "DIORAMA",
            ArtStyle::Solid3D  => "3D",
        }
    }

    pub(crate) fn label_jp(self) -> &'static str {
        match self {
            ArtStyle::Pixel    => "ピクセル",
            ArtStyle::Diorama  => "ジオラマ",
            ArtStyle::Solid3D  => "3D",
        }
    }
}

pub(crate) fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

pub(crate) fn screen_px() -> (f32, f32) {
    (PANEL_X + PANEL_W + MARGIN, MAP_Y + MAP_H + CHROME_H)
}

fn grouped(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn chronicle_parts(line: &str) -> (String, String) {
    if let Some(rest) = line.strip_prefix('Y') {
        if let Some((year, body)) = rest.split_once(' ') {
            if !year.is_empty() && year.chars().all(|c| c.is_ascii_digit()) {
                return (year.to_string(), body.to_string());
            }
        }
    }
    (String::new(), line.to_string())
}

// ---- atlas / library loading -----------------------------------------------

impl Continent {
    /// Loads the terrain atlases and the expanded library atlas from embedded
    /// bytes; called from `Game::start`.
    pub(crate) fn load_atlases(&mut self, assets: &mut Assets) {
        // Primary pixel-art terrain atlas.
        self.atlas = Some(assets.load_png(include_bytes!("../assets/atlas.png")));
        for line in include_str!("../assets/atlas.txt").lines() {
            let mut it = line.split_whitespace();
            let (Some(name), Some(x), Some(y), Some(w), Some(h)) =
                (it.next(), it.next(), it.next(), it.next(), it.next())
            else {
                continue;
            };
            if let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
                x.parse::<f32>(),
                y.parse::<f32>(),
                w.parse::<f32>(),
                h.parse::<f32>(),
            ) {
                for t in ALL_TERRAINS {
                    if t.tile_name() == name {
                        self.terrain_src[t.index()] = Some(Rect::new(x, y, w, h));
                    }
                }
            }
        }

        // Second art style: DQ7-style diorama atlas (toggle with [G]).
        self.atlas_diorama = Some(assets.load_png(include_bytes!("../assets/diorama.png")));
        for line in include_str!("../assets/diorama.txt").lines() {
            let mut it = line.split_whitespace();
            let (Some(name), Some(x), Some(y), Some(w), Some(h)) =
                (it.next(), it.next(), it.next(), it.next(), it.next())
            else {
                continue;
            };
            if let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
                x.parse::<f32>(),
                y.parse::<f32>(),
                w.parse::<f32>(),
                h.parse::<f32>(),
            ) {
                for t in ALL_TERRAINS {
                    if t.tile_name() == name {
                        self.terrain_src_diorama[t.index()] = Some(Rect::new(x, y, w, h));
                    }
                }
            }
        }

        // Expanded library atlas: state-variant tiles keyed by name (pixel mode).
        self.atlas_lib = Some(assets.load_png(include_bytes!("../assets/library.png")));
        for line in include_str!("../assets/library.txt").lines() {
            let mut it = line.split_whitespace();
            let (Some(name), Some(x), Some(y), Some(w), Some(h)) =
                (it.next(), it.next(), it.next(), it.next(), it.next())
            else {
                continue;
            };
            if let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
                x.parse::<f32>(),
                y.parse::<f32>(),
                w.parse::<f32>(),
                h.parse::<f32>(),
            ) {
                self.lib_rects.insert(name.to_string(), Rect::new(x, y, w, h));
            }
        }

        self.atlas_clean = Some(assets.load_png(include_bytes!("../assets/clean.png")));
    }

    fn clean_base_rect(t: Terrain) -> Rect {
        Rect::new(t.index() as f32 * 32.0, 0.0, 32.0, 32.0)
    }

    fn clean_blob_rect(sheet_y: f32, mask: u8) -> Rect {
        let i = autotile_index_4bit(mask);
        let col = (i % 4) as f32;
        let row = (i / 4) as f32;
        Rect::new(col * 32.0, sheet_y + row * 32.0, 32.0, 32.0)
    }

    /// Ocean tile touching land. Mask bits are water neighbours (blob convention).
    fn coast_mask(&self, col: usize, row: usize) -> Option<u8> {
        let t = self.tiles[idx(col, row)].terrain;
        if t != Terrain::Ocean && t != Terrain::DeepOcean {
            return None;
        }
        const DIRS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        let mut mask = 0u8;
        let mut land = false;
        for (bit, (dc, dr)) in DIRS.iter().enumerate() {
            let nc = col as i32 + dc;
            let nr = row as i32 + dr;
            if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                mask |= 1 << bit;
                continue;
            }
            if self.tiles[idx(nc as usize, nr as usize)].terrain.is_water() {
                mask |= 1 << bit;
            } else {
                land = true;
            }
        }
        if land { Some(mask) } else { None }
    }

    /// River tile. Mask bits are river or sea neighbours so the channel meets the coast.
    fn river_mask(&self, col: usize, row: usize) -> Option<u8> {
        if self.tiles[idx(col, row)].terrain != Terrain::River {
            return None;
        }
        const DIRS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        let mut mask = 0u8;
        for (bit, (dc, dr)) in DIRS.iter().enumerate() {
            let nc = col as i32 + dc;
            let nr = row as i32 + dr;
            if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                continue;
            }
            let nt = self.tiles[idx(nc as usize, nr as usize)].terrain;
            if nt == Terrain::River || nt == Terrain::Ocean || nt == Terrain::DeepOcean {
                mask |= 1 << bit;
            }
        }
        Some(mask)
    }

    fn map_sprite(&self, col: usize, row: usize, terrain: Terrain) -> (Option<Texture>, Option<Rect>) {
        match self.art_style {
            ArtStyle::Diorama | ArtStyle::Solid3D => {
                (self.atlas_diorama, self.terrain_src_diorama[terrain.index()])
            }
            ArtStyle::Pixel => {
                if let Some(vr) = self.variant_rect(col, row) {
                    return (self.atlas_lib, Some(vr));
                }
                if let Some(atlas) = self.atlas_clean {
                    let src = if let Some(mask) = self.coast_mask(col, row) {
                        Self::clean_blob_rect(32.0, mask)
                    } else if let Some(mask) = self.river_mask(col, row) {
                        Self::clean_blob_rect(160.0, mask)
                    } else {
                        Self::clean_base_rect(terrain)
                    };
                    return (Some(atlas), Some(src));
                }
                (self.atlas, self.terrain_src[terrain.index()])
            }
        }
    }

    // ---- coordinate helpers ------------------------------------------------

    /// Maps a logical-pixel cursor point to the flat tile index under the cursor,
    /// using the current view_zoom / view_pan.  Returns `None` outside the map
    /// area or outside grid bounds.
    pub(crate) fn tile_at(&self, p: Vec2) -> Option<usize> {
        if p.x < MAP_X || p.x >= MAP_X + MAP_W || p.y < MAP_Y || p.y >= MAP_Y + MAP_H {
            return None;
        }
        let size = TILE * self.view_zoom;
        let col_f = self.view_pan.x + (p.x - MAP_X) / size;
        let row_f = self.view_pan.y + (p.y - MAP_Y) / size;
        if col_f < 0.0 || row_f < 0.0 {
            return None;
        }
        let col = col_f as usize;
        let row = row_f as usize;
        if col >= COLS || row >= ROWS {
            return None;
        }
        Some(idx(col, row))
    }

    /// Returns the screen-space [`Rect`] for tile index `i` using the current
    /// view state.
    pub(crate) fn view_tile_rect(&self, i: usize) -> Rect {
        let col = i % COLS;
        let row = i / COLS;
        let (x, y, size) = view_tile(col, row, self.view_pan, self.view_zoom);
        Rect::new(x, y, size, size)
    }

    // ---- state-variant tile ------------------------------------------------

    /// State-variant tile: in pixel mode, swap in a context-dependent tile from
    /// the expanded library so the map reflects the situation.
    pub(crate) fn variant_rect(&self, col: usize, row: usize) -> Option<Rect> {
        let i = idx(col, row);
        let t = self.tiles[i].terrain;
        if self.scorch[i] > self.year {
            match t {
                Terrain::Forest => {
                    if let Some(r) = self.lib_rects.get("forest-burnt") {
                        return Some(*r);
                    }
                }
                Terrain::Mountain => {
                    if let Some(r) = self.lib_rects.get("mountain-volcanic") {
                        return Some(*r);
                    }
                }
                _ => {}
            }
        }
        if t == Terrain::Plains {
            let i2 = idx(col, row);
            let occ = self.occupied[i2];
            if occ >= 0 && self.settlements[occ as usize].tech & TECH_IRRIGATION != 0 {
                if let Some(&r) = self.lib_rects.get("special-farmland") {
                    return Some(r);
                }
            }
        }
        let cold = row < ROWS * 16 / 100 || row > ROWS * 84 / 100;
        if t == Terrain::Mountain && cold {
            if let Some(r) = self.lib_rects.get("mountain-snowcapped") {
                return Some(*r);
            }
        }
        None
    }

    // ---- road / trade-route helpers ----------------------------------------

    /// Computes road segments between settlements of the same nation that are
    /// within `max_dist` Chebyshev tiles of each other.
    ///
    /// Returns a list of `(tile_a, tile_b)` pairs, deduplicated (A<B index
    /// order) and capped at `MAX_ROAD_SEGMENTS`.  Pure geometry from current
    /// state — no RNG, no sim mutation.
    pub(crate) fn road_segments(&self) -> Vec<(usize, usize)> {
        const MAX_ROAD_DIST: i32 = 6; // Chebyshev distance cap (≈3× COHESION_DIST)
        const MAX_ROAD_SEGMENTS: usize = 256;

        let mut segs: Vec<(usize, usize)> = Vec::new();

        // Iterate over every pair of settlements that share a nation.
        let n = self.settlements.len();
        // nation_of may be shorter than settlements during a cadence gap.
        let nof_len = self.nation_of.len();

        for a in 0..n {
            let nid_a = if a < nof_len { self.nation_of[a] } else { -1 };
            if nid_a < 0 {
                continue;
            }
            let tile_a = self.settlements[a].tile;
            let col_a = (tile_a % COLS) as i32;
            let row_a = (tile_a / COLS) as i32;

            for b in (a + 1)..n {
                if segs.len() >= MAX_ROAD_SEGMENTS {
                    break;
                }
                let nid_b = if b < nof_len { self.nation_of[b] } else { -1 };
                if nid_b != nid_a {
                    continue;
                }
                let tile_b = self.settlements[b].tile;
                let col_b = (tile_b % COLS) as i32;
                let row_b = (tile_b / COLS) as i32;

                let chebyshev = (col_a - col_b).abs().max((row_a - row_b).abs());
                if chebyshev <= MAX_ROAD_DIST {
                    segs.push((tile_a, tile_b));
                }
            }
            if segs.len() >= MAX_ROAD_SEGMENTS {
                break;
            }
        }

        segs
    }

    // ---- farmland helper ---------------------------------------------------

    /// Returns the farmland growth stage (0–3) for tile `i` if it qualifies as
    /// a render-derived farmland tile, otherwise `None`.
    ///
    /// Conditions (all must hold):
    ///   - terrain is `Plains` (the only FARMABLE land type in this sim)
    ///   - tile is not occupied (`occupied[i] < 0`)
    ///   - at least one Chebyshev-distance 1..=2 neighbour is occupied (`>= 0`)
    ///
    /// Stage is deterministic from `(self.year + col_hash + row_hash) % 4`:
    ///   0 = tilled (brown), 1 = growing (green), 2 = ripe (gold), 3 = fallow (tan)
    ///
    /// No RNG, no sim mutation — pure render-derived computation.
    pub(crate) fn farmland_stage(&self, i: usize) -> Option<u8> {
        let tile = self.tiles[i];
        // Only Plains tiles that are unoccupied qualify.
        if tile.terrain != Terrain::Plains {
            return None;
        }
        if self.occupied[i] >= 0 {
            return None;
        }
        let col = (i % COLS) as i32;
        let row = (i / COLS) as i32;
        // Check Chebyshev 1..=2 neighbourhood for a settled tile.
        let mut has_near_settlement = false;
        'outer: for dr in -2i32..=2 {
            for dc in -2i32..=2 {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let chebyshev = dr.abs().max(dc.abs());
                if chebyshev < 1 || chebyshev > 2 {
                    continue;
                }
                let nc = col + dc;
                let nr = row + dr;
                if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                    continue;
                }
                if self.occupied[idx(nc as usize, nr as usize)] >= 0 {
                    has_near_settlement = true;
                    break 'outer;
                }
            }
        }
        if !has_near_settlement {
            return None;
        }
        // Deterministic stage: mix year with per-tile hash of (col, row).
        // Use simple integer mixing — no RNG, pure arithmetic.
        let col_hash = (col as u32).wrapping_mul(0x9e37_79b9);
        let row_hash = (row as u32).wrapping_mul(0x6c62_272e);
        let stage = ((self.year as u32)
            .wrapping_add(col_hash)
            .wrapping_add(row_hash)
            % 4) as u8;
        Some(stage)
    }

    /// Returns the RGBA tint color for a given farmland stage.
    ///
    ///   0 = tilled  → warm brown
    ///   1 = growing → soft green
    ///   2 = ripe    → golden yellow
    ///   3 = fallow  → light tan
    #[inline]
    fn farmland_stage_color(stage: u8) -> Color {
        match stage {
            0 => [0.55, 0.38, 0.22, 0.38], // tilled: brown tint
            1 => [0.42, 0.72, 0.30, 0.35], // growing: green tint
            2 => [0.82, 0.72, 0.18, 0.40], // ripe: gold tint
            _ => [0.72, 0.65, 0.50, 0.32], // fallow: tan tint
        }
    }

    // ---- draw methods -------------------------------------------------------

    pub(crate) fn draw_map(&self, painter: &mut Painter) {
        let zoom = self.view_zoom;
        let pan = self.view_pan;
        let map_right = MAP_X + MAP_W;
        let map_bottom = MAP_Y + MAP_H;

        for row in 0..ROWS {
            for col in 0..COLS {
                let (x, y, size) = view_tile(col, row, pan, zoom);
                if x + size <= MAP_X || x >= map_right || y + size <= MAP_Y || y >= map_bottom {
                    continue;
                }

                let i = idx(col, row);
                let tile = self.tiles[i];

                let dim = self.overlay != Overlay::Terrain && self.overlay != Overlay::Resource;
                let shade = if self.art_style == ArtStyle::Pixel {
                    0.86 + 0.22 * tile.elevation.clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let tint: Color = if dim {
                    [0.5 * shade, 0.5 * shade, 0.5 * shade, 1.0]
                } else {
                    [shade, shade, shade, 1.0]
                };
                let (atlas, src) = self.map_sprite(col, row, tile.terrain);
                match (atlas, src) {
                    (Some(atlas), Some(src)) => {
                        painter.sprite_region(atlas, Rect::new(x, y, size, size), src, tint);
                    }
                    _ => {
                        let mut color = tile.terrain.base_color();
                        if tile.terrain == Terrain::Mountain {
                            let t = ((tile.elevation - MOUNTAIN_LEVEL) / (1.0 - MOUNTAIN_LEVEL))
                                .clamp(0.0, 1.0);
                            color = lerp_color(color, [0.92, 0.93, 0.96, 1.0], t * 0.7);
                        }
                        if dim {
                            color = [color[0] * 0.5, color[1] * 0.5, color[2] * 0.5, 1.0];
                        }
                        painter.rect(x, y, size, size, color);
                    }
                }

                match self.overlay {
                    Overlay::Carrying => {
                        if tile.k > 0.0 {
                            let t = (tile.k / K_MAX).clamp(0.0, 1.0);
                            painter.rect(x, y, size, size, [0.10 + 0.20 * t, 0.20 + 0.75 * t, 0.15, 0.55]);
                        }
                    }
                    Overlay::Population => {
                        let occ = self.occupied[i];
                        if occ >= 0 {
                            let s = &self.settlements[occ as usize];
                            let frac = if tile.k > 0.0 {
                                (s.pop / tile.k).clamp(0.0, 1.0)
                            } else {
                                0.0
                            };
                            painter.rect(x, y, size, size, [0.85, 0.25 + 0.45 * (1.0 - frac), 0.12, 0.55]);
                        }
                    }
                    Overlay::Nation => {
                        let occ = self.occupied[i];
                        if occ >= 0 {
                            let nid = self.nation_of.get(occ as usize).copied().unwrap_or(-1);
                            if nid >= 0 {
                                if let Some(nation) = self.nations.iter().find(|n| n.id as i32 == nid) {
                                    let color = nation.color;
                                    painter.rect(x, y, size, size, [color[0], color[1], color[2], 0.5]);
                                }
                            }
                        }
                    }
                    Overlay::Tech => {
                        let occ = self.occupied[i];
                        if occ >= 0 {
                            let t = self.settlements[occ as usize].tech.count_ones() as f32 / 4.0;
                            painter.rect(x, y, size, size, [0.2 + 0.7 * t, 0.2 + 0.7 * t, 0.6 * (1.0 - t), 0.5]);
                        }
                    }
                    Overlay::Resource => {
                        if i < self.resources.len() {
                            let tint_color: Option<Color> = match self.resources[i] {
                                Resource::Iron   => Some([0.60, 0.65, 0.70, 0.55]),
                                Resource::Horses => Some([0.55, 0.35, 0.15, 0.55]),
                                Resource::Gold   => Some([0.90, 0.80, 0.10, 0.55]),
                                Resource::None   => None,
                            };
                            if let Some(tc) = tint_color {
                                painter.rect(x, y, size, size, tc);
                            }
                        }
                    }
                    Overlay::Terrain => {}
                }
            }
        }

        // Realm outline: only the edge that faces another people, or the wild.
        if self.overlay != Overlay::Carrying && self.overlay != Overlay::Resource {
            let mut owner = vec![-1i32; self.tiles.len()];
            let mut tint: Vec<Color> = vec![[0.0, 0.0, 0.0, 0.0]; self.tiles.len()];
            for nation in &self.nations {
                let border = [nation.color[0], nation.color[1], nation.color[2], 0.92];
                for &si in &nation.territory {
                    let Some(s) = self.settlements.get(si) else { continue };
                    if s.tile < owner.len() {
                        owner[s.tile] = nation.id as i32;
                        tint[s.tile] = border;
                    }
                }
            }
            let edges: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];
            for tile_idx in 0..owner.len() {
                let id = owner[tile_idx];
                if id < 0 {
                    continue;
                }
                let col = tile_idx % COLS;
                let row = tile_idx / COLS;
                let (x, y, size) = view_tile(col, row, pan, zoom);
                if x + size <= MAP_X || x >= map_right || y + size <= MAP_Y || y >= map_bottom {
                    continue;
                }
                let border = tint[tile_idx];
                let b = 1.5;
                for (k, (dc, dr)) in edges.iter().enumerate() {
                    let nc = col as i32 + dc;
                    let nr = row as i32 + dr;
                    let foreign = if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                        true
                    } else {
                        owner[idx(nc as usize, nr as usize)] != id
                    };
                    if !foreign {
                        continue;
                    }
                    match k {
                        0 => painter.rect(x, y, size, b, border),
                        1 => painter.rect(x, y + size - b, size, b, border),
                        2 => painter.rect(x, y, b, size, border),
                        _ => painter.rect(x + size - b, y, b, size, border),
                    }
                }
            }
        }

        self.draw_wildlife(painter, pan, zoom, map_right, map_bottom);

        // ---- farmland overlay (drawn AFTER base tiles, BEFORE roads/markers) ----
        // For each Plains tile near a settlement (Chebyshev 1..=2), draw a
        // subtle stage-colored tint and two horizontal crop stripes.  Pure
        // render-derived — no sim state mutation.
        if self.overlay == Overlay::Terrain || self.overlay == Overlay::Resource {
            for row in 0..ROWS {
                for col in 0..COLS {
                    let i = idx(col, row);
                    let Some(stage) = self.farmland_stage(i) else {
                        continue;
                    };
                    let (x, y, size) = view_tile(col, row, pan, zoom);
                    if x + size <= MAP_X || x >= map_right || y + size <= MAP_Y || y >= map_bottom {
                        continue;
                    }
                    // Stage-colored tint over the base tile.
                    let tint = Self::farmland_stage_color(stage);
                    painter.rect(x, y, size, size, tint);
                    // Two horizontal crop stripes (darker, semi-transparent).
                    let stripe_h = (size * 0.10).max(1.0);
                    let stripe_alpha = 0.25_f32;
                    let stripe_col: Color = [
                        tint[0] * 0.65,
                        tint[1] * 0.65,
                        tint[2] * 0.65,
                        stripe_alpha,
                    ];
                    // Stripe at ~30% and ~65% from the top of the tile.
                    painter.rect(x, y + size * 0.28, size, stripe_h, stripe_col);
                    painter.rect(x, y + size * 0.62, size, stripe_h, stripe_col);
                }
            }
        }

        // ---- road / trade-route segments (drawn BEFORE settlement markers) ----
        // Brown/tan line segments between same-nation settlements within range.
        // Rendered as a DDA-traced sequence of small square dots so lines appear
        // at any angle without requiring rotate-rect support in the Painter.
        {
            const ROAD_COLOR: Color = [0.55, 0.38, 0.18, 0.55]; // brown/tan, semi-transparent
            let road_dot = (TILE * zoom * 0.22).max(1.5); // dot size scales with zoom
            let road_half = road_dot * 0.5;

            for (tile_a, tile_b) in self.road_segments() {
                let col_a = tile_a % COLS;
                let row_a = tile_a / COLS;
                let col_b = tile_b % COLS;
                let row_b = tile_b / COLS;
                let (tx_a, ty_a, tsize_a) = view_tile(col_a, row_a, pan, zoom);
                let (tx_b, ty_b, tsize_b) = view_tile(col_b, row_b, pan, zoom);
                let ax = tx_a + tsize_a * 0.5;
                let ay = ty_a + tsize_a * 0.5;
                let bx = tx_b + tsize_b * 0.5;
                let by = ty_b + tsize_b * 0.5;

                // Skip entirely off-screen segments (loose bound check).
                let min_x = ax.min(bx);
                let max_x = ax.max(bx);
                let min_y = ay.min(by);
                let max_y = ay.max(by);
                if max_x < MAP_X || min_x > map_right || max_y < MAP_Y || min_y > map_bottom {
                    continue;
                }

                // DDA: step along the longer axis, plot dots every ~1/3 tile.
                let dx = bx - ax;
                let dy = by - ay;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                let dot_spacing = (TILE * zoom * 0.30).max(2.0);
                let steps = (dist / dot_spacing).ceil() as u32;
                for s in 0..=steps {
                    let t = s as f32 / steps.max(1) as f32;
                    let px = ax + dx * t;
                    let py = ay + dy * t;
                    // Clip individual dots to map viewport.
                    if px + road_half < MAP_X || px - road_half > map_right
                        || py + road_half < MAP_Y || py - road_half > map_bottom
                    {
                        continue;
                    }
                    painter.rect(px - road_half, py - road_half, road_dot, road_dot, ROAD_COLOR);
                }
            }
        }

        // ---- ships (2D: Pixel + Diorama) ----------------------------------------
        // Small boat markers on the water under the view transform.
        // Drawn after roads, before settlement markers so boats appear under
        // the settlement dots.  Uses the same ship_positions() helper as 3D.
        {
            const SHIP_DOT_COLOR: Color = [0.28, 0.15, 0.06, 0.90]; // dark brown
            const SAIL_DOT_COLOR: Color = [0.95, 0.92, 0.85, 0.85]; // off-white sail

            for (wx, wz, _heading) in self.ship_positions() {
                // Convert 3D world (wx, wz) → 2D screen.
                // world_x = col + 0.5  →  screen_x = MAP_X + (wx - pan.x) * (TILE * zoom)
                let sx = MAP_X + (wx - pan.x) * (TILE * zoom);
                let sy = MAP_Y + (wz - pan.y) * (TILE * zoom);

                // Skip if off-screen.
                if sx < MAP_X - 4.0 || sx > map_right + 4.0
                    || sy < MAP_Y - 4.0 || sy > map_bottom + 4.0
                {
                    continue;
                }

                // Hull: a small horizontal rectangle.
                let hw = (TILE * zoom * 0.30).max(2.0);
                let hh = (TILE * zoom * 0.12).max(1.0);
                painter.rect(sx - hw * 0.5, sy - hh * 0.5, hw, hh, SHIP_DOT_COLOR);

                // Sail: a small square above the hull centre.
                let sw = (TILE * zoom * 0.14).max(1.5);
                painter.rect(sx - sw * 0.5, sy - hh * 0.5 - sw, sw, sw, SAIL_DOT_COLOR);
            }
        }

        if self.overlay != Overlay::Population {
            // One figure per people. Everyone else is a point of light, not a letter.
            let mut capital = vec![false; self.settlements.len()];
            for nation in &self.nations {
                let mut best: Option<(usize, f32)> = None;
                for &si in &nation.territory {
                    let Some(s) = self.settlements.get(si) else { continue };
                    if best.map(|(_, p)| s.pop > p).unwrap_or(true) {
                        best = Some((si, s.pop));
                    }
                }
                if let Some((si, _)) = best {
                    capital[si] = true;
                }
            }
            for (si, s) in self.settlements.iter().enumerate() {
                let col = s.tile % COLS;
                let row = s.tile / COLS;
                let (tx, ty, tsize) = view_tile(col, row, pan, zoom);
                if tx + tsize <= MAP_X || tx >= map_right || ty + tsize <= MAP_Y || ty >= map_bottom {
                    continue;
                }
                let nid = self.nation_of.get(si).copied().unwrap_or(-1);
                let mark = if nid >= 0 {
                    self.nations
                        .iter()
                        .find(|n| n.id as i32 == nid)
                        .map(|n| [n.color[0], n.color[1], n.color[2], 0.95])
                        .unwrap_or([0.93, 0.90, 0.84, 0.7])
                } else {
                    [0.93, 0.90, 0.84, 0.55]
                };
                let major = s.city || (capital[si] && s.pop >= 60.0);
                if major {
                    let h = if s.city { 9.0 } else { 6.0 };
                    let w = 2.0;
                    let bx = tx + (tsize - w) * 0.5;
                    let by = ty + (tsize - h) * 0.5;
                    painter.rect(bx, by + 2.0, w, (h - 2.0).max(2.0), mark);
                    painter.rect(bx - 1.0, by, w + 2.0, 2.0, mark);
                } else if s.pop >= 22.0 {
                    let d = if s.city { 3.0 } else { 2.0 };
                    painter.rect(tx + (tsize - d) * 0.5, ty + (tsize - d) * 0.5, d, d, mark);
                }
            }
        }

        // §5: disaster epicentre tint.
        if let Some((epi_tile, _kind, epi_year)) = self.last_disaster {
            let age = self.year.saturating_sub(epi_year);
            if age < DISASTER_TINT_YEARS {
                let fade = 1.0 - age as f32 / DISASTER_TINT_YEARS as f32;
                let r = self.view_tile_rect(epi_tile);
                if r.x + r.w > MAP_X && r.x < map_right && r.y + r.h > MAP_Y && r.y < map_bottom {
                    painter.rect(r.x, r.y, r.w, r.h, [1.0, 0.25, 0.10, 0.55 * fade]);
                }
            }
        }

        // §9: war front borders.
        for war in &self.wars {
            for &front_tile in &war.fronts {
                let r = self.view_tile_rect(front_tile);
                if r.x + r.w <= MAP_X || r.x >= map_right || r.y + r.h <= MAP_Y || r.y >= map_bottom {
                    continue;
                }
                painter.rect(r.x, r.y, r.w, 1.5, [0.95, 0.15, 0.15, 0.80]);
                painter.rect(r.x, r.y + r.h - 1.5, r.w, 1.5, [0.95, 0.15, 0.15, 0.80]);
                painter.rect(r.x, r.y, 1.5, r.h, [0.95, 0.15, 0.15, 0.80]);
                painter.rect(r.x + r.w - 1.5, r.y, 1.5, r.h, [0.95, 0.15, 0.15, 0.80]);
            }
        }

        // §8 event-flash: fading ring overlay.
        {
            let year = self.year;
            for &(tile, started) in &self.flashes {
                if tile >= self.tiles.len() {
                    continue;
                }
                let age = year.saturating_sub(started);
                if age >= FLASH_YEARS {
                    continue;
                }
                let r = self.view_tile_rect(tile);
                if r.x + r.w <= MAP_X || r.x >= map_right || r.y + r.h <= MAP_Y || r.y >= map_bottom {
                    continue;
                }
                let alpha = 0.85 * (1.0 - age as f32 / FLASH_YEARS as f32);
                let ring_w = (r.w * 0.18).max(1.5);
                let col: Color = [1.0, 0.90, 0.30, alpha];
                painter.rect(r.x, r.y, r.w, ring_w, col);
                painter.rect(r.x, r.y + r.h - ring_w, r.w, ring_w, col);
                painter.rect(r.x, r.y, ring_w, r.h, col);
                painter.rect(r.x + r.w - ring_w, r.y, ring_w, r.h, col);
            }
        }

        // Hover highlight while editing, or while a god tool other than inspect is armed.
        if self.edit_mode == EditMode::Edit || self.god_tool != GodTool::Inspect {
            if let Some(hi) = self.hovered_tile {
                let r = self.view_tile_rect(hi);
                painter.rect(r.x, r.y, r.w, 1.5, [1.0, 1.0, 0.6, 0.90]);
                painter.rect(r.x, r.y + r.h - 1.5, r.w, 1.5, [1.0, 1.0, 0.6, 0.90]);
                painter.rect(r.x, r.y, 1.5, r.h, [1.0, 1.0, 0.6, 0.90]);
                painter.rect(r.x + r.w - 1.5, r.y, 1.5, r.h, [1.0, 1.0, 0.6, 0.90]);
            }
        }
    }

    fn draw_wildlife(&self, painter: &mut Painter, pan: Vec2, zoom: f32, map_right: f32, map_bottom: f32) {
        let Some(lib) = self.atlas_lib else { return };
        for row in 0..ROWS {
            for col in 0..COLS {
                let i = idx(col, row);
                if i % 23 != 0 || self.occupied[i] >= 0 {
                    continue;
                }
                let t = self.tiles[i].terrain;
                let name = match t {
                    Terrain::Forest => if i % 46 == 0 { "animal-deer" } else { "animal-boar" },
                    Terrain::Plains => {
                        if i < self.resources.len() && self.resources[i] == Resource::Horses {
                            "animal-horse"
                        } else if i % 46 == 0 {
                            "animal-cattle"
                        } else {
                            continue;
                        }
                    }
                    Terrain::Desert => "animal-wolf",
                    Terrain::Ocean | Terrain::DeepOcean => {
                        let coastal = NEIGHBORS8.iter().any(|&(dc, dr)| {
                            let nc = col as i32 + dc;
                            let nr = row as i32 + dr;
                            if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                                return false;
                            }
                            !self.tiles[idx(nc as usize, nr as usize)].terrain.is_water()
                        });
                        if coastal { "animal-fish" } else { continue }
                    }
                    _ => continue,
                };
                let Some(&src) = self.lib_rects.get(name) else { continue };
                let (x, y, size) = view_tile(col, row, pan, zoom);
                if x + size <= MAP_X || x >= map_right || y + size <= MAP_Y || y >= map_bottom {
                    continue;
                }
                let m = size * 0.72;
                painter.sprite_region(
                    lib,
                    Rect::new(x + (size - m) * 0.5, y + (size - m) * 0.5, m, m),
                    src,
                    [1.0, 1.0, 1.0, 1.0],
                );
            }
        }
    }

    pub(crate) fn tx(
        &self,
        painter: &mut Painter,
        x: f32,
        y: f32,
        px: f32,
        color: Color,
        text: &str,
    ) {
        if text.is_empty() {
            return;
        }
        if let Some(font) = self.font.as_ref() {
            painter.text_font(x, y, px, color, font, text);
        } else {
            painter.text(x, y, px / FONT_GLYPH_H, color, text);
        }
    }

    pub(crate) fn tx_w(&self, painter: &mut Painter, px: f32, text: &str) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        if let Some(font) = self.font.as_ref() {
            painter.text_font_width(px, font, text)
        } else {
            text.chars().count() as f32 * 8.0 * (px / FONT_GLYPH_H)
        }
    }

    fn tx_centered(
        &self,
        painter: &mut Painter,
        cx: f32,
        y: f32,
        px: f32,
        color: Color,
        text: &str,
    ) {
        let w = self.tx_w(painter, px, text);
        self.tx(painter, cx - w * 0.5, y, px, color, text);
    }

    fn fit_line(&self, painter: &mut Painter, px: f32, text: &str, max_w: f32) -> String {
        if text.is_empty() || self.tx_w(painter, px, text) <= max_w {
            return text.to_string();
        }
        let chars: Vec<char> = text.chars().collect();
        let mut n = chars.len();
        while n > 0 {
            let head: String = chars[..n].iter().collect();
            let shown = format!("{head}...");
            if self.tx_w(painter, px, &shown) <= max_w {
                return shown;
            }
            n -= 1;
        }
        String::new()
    }

    fn stat_line(
        &self,
        painter: &mut Painter,
        x: f32,
        y: f32,
        w: f32,
        label: &str,
        value: &str,
    ) -> f32 {
        let px = 12.0;
        self.tx(painter, x, y, px, UI_DIM, label);
        let label_w = self.tx_w(painter, px, label) + 10.0;
        let shown = self.fit_line(painter, px, value, (w - label_w).max(28.0));
        let vw = self.tx_w(painter, px, &shown);
        self.tx(painter, (x + w - vw).max(x + label_w), y, px, UI_INK, &shown);
        y + 16.0
    }

    fn unification_frac(&self) -> f32 {
        if self.unification_logged {
            return 1.0;
        }
        if self.extinction_logged {
            return 0.0;
        }
        let n = self.settlements.len();
        if n == 0 {
            return 0.0;
        }
        let best = self
            .nations
            .iter()
            .map(|nat| nat.territory.len())
            .max()
            .unwrap_or(0);
        (best as f32 / n as f32 / UNIFICATION_THRESHOLD).clamp(0.0, 1.0)
    }

    fn paint_frame(&self, painter: &mut Painter) {
        let (sw, sh) = screen_px();
        painter.rect(0.0, 0.0, sw, MAP_Y, UI_PAGE);
        painter.rect(0.0, 0.0, MAP_X, sh, UI_PAGE);
        painter.rect(MAP_X + MAP_W, MAP_Y, MARGIN, MAP_H, UI_PAGE);
        painter.rect(PANEL_X + PANEL_W, 0.0, MARGIN, sh, UI_PAGE);
    }

    fn stroke_series(
        painter: &mut Painter,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        samples: &[f32],
        max_v: f32,
        color: Color,
    ) {
        if samples.is_empty() || max_v <= 0.0 {
            return;
        }
        let n = samples.len();
        let step = w / (n as f32).max(8.0);
        for (i, &v) in samples.iter().enumerate() {
            let t = (v / max_v).clamp(0.0, 1.0);
            let ny = y + h - (h * t).max(1.0);
            painter.rect(x + i as f32 * step, ny, (step + 0.6).max(1.5), 1.5, color);
        }
    }

    pub(crate) fn draw_panel(&self, painter: &mut Painter) {
        self.paint_frame(painter);
        painter.rect(PANEL_X, MAP_Y, PANEL_W, MAP_H, UI_PANEL);
        painter.rect(
            PANEL_X,
            MAP_Y,
            1.0,
            MAP_H,
            [UI_GOLD[0], UI_GOLD[1], UI_GOLD[2], 0.45],
        );

        let x = PANEL_X + 14.0;
        let inner_w = PANEL_W - 26.0;
        let mut y = MAP_Y + 18.0;

        let year = self.year.to_string();
        self.tx(painter, x, y, 36.0, UI_GOLD, &year);
        let year_w = self.tx_w(painter, 36.0, &year);
        self.tx(painter, x + year_w + 6.0, y + 18.0, 13.0, UI_DIM, "年");
        y += 44.0;
        self.tx(painter, x, y, 12.0, UI_DIM, self.current_era().label_jp());
        y += 22.0;

        let pop_total: f32 = self.settlements.iter().map(|s| s.pop).sum();
        let colonized = if self.habitable_count > 0 {
            100.0 * self.settlements.len() as f32 / self.habitable_count as f32
        } else {
            0.0
        };
        let land = self
            .nations
            .iter()
            .max_by_key(|n| n.territory.len())
            .map(|n| (n.id, n.name.clone(), self.nation_total_pop(n) as u64));
        let might = self
            .nations
            .iter()
            .max_by(|a, b| {
                self.nation_total_pop(a)
                    .partial_cmp(&self.nation_total_pop(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|n| (n.id, n.name.clone(), self.nation_total_pop(n) as u64));
        let wars = if self.wars.is_empty() {
            "なし".to_string()
        } else if let Some(w) = self.wars.iter().max_by_key(|w| w.fronts.len()) {
            let na = self
                .nations
                .iter()
                .find(|n| n.id == w.a)
                .map(|n| n.name.as_str())
                .unwrap_or("?");
            let nb = self
                .nations
                .iter()
                .find(|n| n.id == w.b)
                .map(|n| n.name.as_str())
                .unwrap_or("?");
            format!("{}件 · {} 対 {}", self.wars.len(), na, nb)
        } else {
            format!("{}件", self.wars.len())
        };
        let disaster = if let Some((_, kind, yr)) = self.last_disaster {
            format!("{}件 · {} {}年", self.disaster_count, kind.label_jp(), yr)
        } else if self.disaster_count == 0 {
            "なし".to_string()
        } else {
            format!("{}件", self.disaster_count)
        };
        let tech = Continent::tech_display(self.all_tech());
        let traits = if let Some((_, ref name, _)) = land {
            self.nations
                .iter()
                .find(|n| n.name == *name)
                .map(|n| traits_display(n.traits))
                .unwrap_or_else(|| "なし".to_string())
        } else {
            "なし".to_string()
        };

        y = self.stat_line(painter, x, y, inner_w, "人口", &grouped(pop_total as u64));
        y = self.stat_line(
            painter,
            x,
            y,
            inner_w,
            "集落",
            &grouped(self.settlements.len() as u64),
        );
        y = self.stat_line(painter, x, y, inner_w, "国家", &self.nations.len().to_string());
        y = self.stat_line(painter, x, y, inner_w, "開拓", &format!("{:.0}%", colonized));
        if let Some((_, name, pop)) = &land {
            y = self.stat_line(
                painter,
                x,
                y,
                inner_w,
                "最大",
                &format!("{name} {}", grouped(*pop)),
            );
        } else {
            y = self.stat_line(painter, x, y, inner_w, "最大", "なし");
        }
        if let (Some((lid, _, _)), Some((mid, mname, mpop))) = (&land, &might) {
            if lid != mid {
                y = self.stat_line(
                    painter,
                    x,
                    y,
                    inner_w,
                    "最強",
                    &format!("{mname} {}", grouped(*mpop)),
                );
            }
        }
        y = self.stat_line(painter, x, y, inner_w, "戦争", &wars);
        y = self.stat_line(painter, x, y, inner_w, "災害", &disaster);
        y = self.stat_line(painter, x, y, inner_w, "技術", &tech);
        y = self.stat_line(painter, x, y, inner_w, "特性", &traits);
        y += 8.0;

        let goal = self.goal_line();
        self.tx(painter, x, y, 13.0, UI_GOLD, &goal);
        y += 16.0;
        let frac = self.unification_frac();
        painter.rect(x, y, inner_w, 2.0, UI_TRACK);
        painter.rect(x, y, inner_w * frac, 2.0, UI_GOLD);
        y += 16.0;

        let legend_n = "国家";
        let legend_pop = "人口";
        let ln_w = self.tx_w(painter, 11.0, legend_n);
        let lp_w = self.tx_w(painter, 11.0, legend_pop);
        self.tx(painter, x + inner_w - ln_w - 12.0 - lp_w, y, 11.0, UI_DIM, legend_pop);
        self.tx(painter, x + inner_w - ln_w, y, 11.0, UI_DIM, legend_n);
        y += 14.0;

        let plot_h = 40.0;
        if self.hist_pop.is_empty() {
            painter.rect(x, y + plot_h - 1.0, inner_w, 1.0, UI_TRACK);
            self.tx(painter, x, y + 12.0, 12.0, UI_DIM, "まだ推移はない");
        } else {
            let max_pop = self.hist_pop.iter().cloned().fold(1.0f32, f32::max) * 1.18;
            Self::stroke_series(
                painter,
                x,
                y,
                inner_w,
                plot_h,
                &self.hist_pop,
                max_pop,
                UI_GOLD,
            );
            if !self.hist_nations.is_empty() {
                let nations: Vec<f32> = self.hist_nations.iter().map(|n| *n as f32).collect();
                let max_n = nations.iter().cloned().fold(1.0f32, f32::max) * 1.18;
                Self::stroke_series(
                    painter,
                    x,
                    y,
                    inner_w,
                    plot_h,
                    &nations,
                    max_n,
                    [UI_DIM[0], UI_DIM[1], UI_DIM[2], 0.85],
                );
            }
        }
        y += plot_h + 16.0;

        self.tx(painter, x, y, 11.0, UI_DIM, "年代記");
        y += 16.0;
        let list_bot = MAP_Y + MAP_H - 12.0;
        let row_h = 16.0;
        let slots = ((list_bot - y) / row_h).floor().max(0.0) as usize;
        if self.log.is_empty() {
            self.tx(painter, x, y, 12.0, UI_DIM, "まだ記録はない");
        } else if slots > 0 {
            let start = self.log.len().saturating_sub(slots);
            let shown = self.log.len() - start;
            painter.rect(
                x + 26.0,
                y + 2.0,
                1.0,
                (shown as f32 * row_h - 6.0).max(1.0),
                [UI_DIM[0], UI_DIM[1], UI_DIM[2], 0.28],
            );
            for (i, line) in self.log.iter().skip(start).enumerate() {
                let ly = y + i as f32 * row_h;
                let (year, body) = chronicle_parts(line);
                if year.is_empty() {
                    let shown = self.fit_line(painter, 12.0, &body, inner_w);
                    self.tx(painter, x, ly, 12.0, UI_INK, &shown);
                } else {
                    let yw = self.tx_w(painter, 11.0, &year);
                    self.tx(painter, x + 20.0 - yw, ly + 1.0, 11.0, UI_DIM, &year);
                    let shown = self.fit_line(painter, 12.0, &body, inner_w - 32.0);
                    self.tx(painter, x + 32.0, ly, 12.0, UI_INK, &shown);
                }
            }
        }
    }

    pub(crate) fn draw_status(&self, painter: &mut Painter) {
        let (sw, _) = screen_px();
        let y0 = MAP_Y + MAP_H;
        painter.rect(0.0, y0, sw, CHROME_H, UI_DOCK);
        painter.rect(0.0, y0, sw, 1.0, UI_GOLD);

        let py = 13.0;
        let y = y0 + 14.0;
        let mut x = 16.0;

        let pause = if self.paused { "停止中" } else { "停止" };
        let pause_c = if self.paused { UI_GOLD } else { UI_DIM };
        self.tx(painter, x, y, py, pause_c, pause);
        x += self.tx_w(painter, py, pause) + 6.0;
        self.tx(painter, x, y + 1.0, 11.0, UI_DIM, "Space");
        x += self.tx_w(painter, 11.0, "Space") + 20.0;

        for s in 1..=4u8 {
            let label = s.to_string();
            let c = if self.speed == s { UI_INK } else { UI_DIM };
            self.tx(painter, x, y, py, c, &label);
            x += self.tx_w(painter, py, &label) + 11.0;
        }
        x += 12.0;

        let tools = [
            (GodTool::Inspect, "5"),
            (GodTool::Raise, "6"),
            (GodTool::Grace, "7"),
            (GodTool::Found, "8"),
        ];
        for (tool, key) in tools {
            let on = self.god_tool == tool;
            self.tx(painter, x, y + 1.0, 11.0, UI_DIM, key);
            x += self.tx_w(painter, 11.0, key) + 4.0;
            let name = tool.label_jp();
            self.tx(painter, x, y, py, if on { UI_GOLD } else { UI_DIM }, name);
            x += self.tx_w(painter, py, name) + 14.0;
        }
        x += 6.0;

        let overlay = self.overlay.label_jp();
        self.tx(painter, x, y, py, UI_DIM, overlay);
        x += self.tx_w(painter, py, overlay) + 4.0;
        self.tx(painter, x, y + 1.0, 11.0, UI_DIM, "O");
        x += self.tx_w(painter, 11.0, "O") + 14.0;

        let art_name = if self.font.is_some() {
            self.art_style.label_jp()
        } else {
            self.art_style.label()
        };
        self.tx(painter, x, y, py, UI_DIM, art_name);
        x += self.tx_w(painter, py, art_name) + 4.0;
        self.tx(painter, x, y + 1.0, 11.0, UI_DIM, "G");
        x += self.tx_w(painter, 11.0, "G") + 14.0;

        self.tx(painter, x, y, py, UI_DIM, "ヘルプ");
        x += self.tx_w(painter, py, "ヘルプ") + 4.0;
        self.tx(painter, x, y + 1.0, 11.0, UI_DIM, "H");
        x += self.tx_w(painter, 11.0, "H") + 12.0;

        let faith_num = format!("{:.0}", self.faith);
        let faith_num_w = self.tx_w(painter, py, &faith_num);
        let mut right = sw - 16.0;
        self.tx(painter, right - faith_num_w, y, py, UI_DIM, &faith_num);
        right -= faith_num_w + 8.0;
        let bar_w = 48.0;
        let frac = (self.faith / FAITH_MAX).clamp(0.0, 1.0);
        painter.rect(right - bar_w, y + 6.0, bar_w, 2.0, UI_TRACK);
        let bar_c = if self.faith < 10.0 {
            UI_RED
        } else {
            [UI_DIM[0], UI_DIM[1], UI_DIM[2], 0.9]
        };
        painter.rect(right - bar_w, y + 6.0, bar_w * frac, 2.0, bar_c);
        right -= bar_w + 8.0;
        let faith_w = self.tx_w(painter, py, "信仰");
        self.tx(painter, right - faith_w, y, py, UI_DIM, "信仰");
        right -= faith_w + 18.0;

        if self.art_style == ArtStyle::Solid3D {
            let cam = "Q E 回転   Z X 仰角";
            let cw = self.tx_w(painter, 12.0, cam);
            if right - cw > x + 8.0 {
                self.tx(painter, right - cw, y + 1.0, 12.0, UI_DIM, cam);
            }
        }
    }

    pub(crate) fn draw_outcome(&self, painter: &mut Painter) {
        let (title, accent): (&str, Color) = if self.extinction_logged {
            ("人々は滅びた", UI_RED)
        } else if self.unification_logged {
            ("大陸は統一された", UI_GOLD)
        } else {
            return;
        };
        let bh = 128.0;
        let by = MAP_Y + (MAP_H - bh) * 0.40;
        painter.rect(MAP_X, by, MAP_W, bh, [0.008, 0.010, 0.018, 0.82]);
        painter.rect(MAP_X, by, MAP_W, 1.0, accent);
        painter.rect(MAP_X, by + bh - 1.0, MAP_W, 1.0, accent);
        let cx = MAP_X + MAP_W * 0.5;
        self.tx_centered(painter, cx, by + 34.0, 32.0, accent, title);
        self.tx_centered(painter, cx, by + 78.0, 14.0, UI_DIM, "時間は止まっています");
    }

    pub(crate) fn draw_selection_panel(&self, painter: &mut Painter) {
        let Some(ti) = self.selected_tile else {
            return;
        };
        let col = ti % COLS;
        let row = ti / COLS;
        let (hx, hy, hsize) = view_tile(col, row, self.view_pan, self.view_zoom);
        let b = 1.5;
        painter.rect(hx, hy, hsize, b, UI_GOLD);
        painter.rect(hx, hy + hsize - b, hsize, b, UI_GOLD);
        painter.rect(hx, hy, b, hsize, UI_GOLD);
        painter.rect(hx + hsize - b, hy, b, hsize, UI_GOLD);

        let tile = self.tiles[ti];
        let mut lines: Vec<(String, String)> = Vec::new();
        lines.push((
            "地形".to_string(),
            tile.terrain.tile_name_jp().to_string(),
        ));
        lines.push((
            "収容".to_string(),
            format!(
                "{:.0} · {}",
                tile.k,
                if tile.habitable { "居住可" } else { "不毛" }
            ),
        ));
        if ti < self.resources.len() && self.resources[ti] != Resource::None {
            lines.push(("資源".to_string(), self.resources[ti].label_jp().to_string()));
        }

        let si = self.occupied[ti];
        if si >= 0 {
            let s = &self.settlements[si as usize];
            let kind = if s.city { "都市" } else { "集落" };
            let mut pop = grouped(s.pop as u64);
            if s.stuck > 0 {
                pop.push_str(&format!(" · 停滞 {}", s.stuck));
            }
            if s.inspired_years > 0 {
                pop.push_str(" · 鼓舞");
            }
            lines.push(("人".to_string(), format!("{kind} · {pop}")));
            lines.push(("技術".to_string(), Continent::tech_display(s.tech)));
            lines.push(("特性".to_string(), traits_display(s.traits)));
            let nid = self.nation_of.get(si as usize).copied().unwrap_or(-1);
            if nid >= 0 {
                if let Some(n) = self.nations.iter().find(|n| n.id as i32 == nid) {
                    let total_pop = self.nation_total_pop(n) as u64;
                    let mut banner = format!(
                        "{} · {}集落 · {}人",
                        n.name,
                        n.territory.len(),
                        grouped(total_pop)
                    );
                    if n.golden {
                        banner.push_str(" · 黄金期");
                    }
                    lines.push(("国家".to_string(), banner));
                    lines.push(("文化".to_string(), n.culture.to_string()));
                    lines.push(("交易".to_string(), format!("{}か国", n.trade_partners)));
                }
            } else {
                lines.push(("国家".to_string(), "無国籍".to_string()));
            }
        } else {
            lines.push(("人".to_string(), "未開地".to_string()));
        }

        let bw = 268.0;
        let row_h = 16.0;
        let bh = 40.0 + lines.len() as f32 * row_h;
        let bx = MAP_X + 16.0;
        let by = MAP_Y + MAP_H - bh - 14.0;
        painter.rect(bx, by, bw, bh, [0.008, 0.010, 0.018, 0.88]);
        painter.rect(bx, by, 2.0, bh, UI_GOLD);
        self.tx(painter, bx + 14.0, by + 10.0, 14.0, UI_GOLD, "調査");
        let coord = format!("{col},{row}");
        let cw = self.tx_w(painter, 11.0, &coord);
        self.tx(painter, bx + bw - 12.0 - cw, by + 12.0, 11.0, UI_DIM, &coord);

        let mut ly = by + 32.0;
        for (label, value) in &lines {
            self.tx(painter, bx + 14.0, ly, 12.0, UI_DIM, label);
            let shown = self.fit_line(painter, 12.0, value, bw - 78.0);
            self.tx(painter, bx + 58.0, ly, 12.0, UI_INK, &shown);
            ly += row_h;
        }
    }

    fn help_column(&self, painter: &mut Painter, x: f32, mut y: f32, rows: &[(&str, &str)]) {
        for (key, val) in rows {
            if key.is_empty() {
                y += 8.0;
                self.tx(painter, x, y, 12.0, UI_DIM, val);
                y += 20.0;
            } else {
                self.tx(painter, x, y, 14.0, UI_INK, key);
                self.tx(painter, x + 108.0, y, 14.0, UI_DIM, val);
                y += 24.0;
            }
        }
    }

    pub(crate) fn draw_help_overlay(&self, painter: &mut Painter) {
        let (sw, sh) = screen_px();
        painter.rect(0.0, 0.0, sw, sh, [0.008, 0.010, 0.018, 0.94]);
        let x = 72.0;
        let y = 56.0;
        self.tx(painter, x, y, 36.0, UI_INK, "操作");
        painter.rect(x, y + 48.0, 52.0, 1.0, UI_GOLD);
        let close = "H  閉じる";
        let cw = self.tx_w(painter, 14.0, close);
        self.tx(painter, sw - 72.0 - cw, y + 12.0, 14.0, UI_DIM, close);

        let left: &[(&str, &str)] = &[
            ("", "時間"),
            ("Space", "停止と再開"),
            ("1-4", "速度"),
            ("", "神の手"),
            ("5", "調べる"),
            ("6", "陸を上げる"),
            ("7", "恵みを与える"),
            ("8", "建国する"),
            ("クリック", "その場所で使う"),
            ("", "画面"),
            ("O", "表示を切り替える"),
            ("G", "画風を切り替える"),
            ("+ -", "ズーム"),
            ("矢印", "地図を動かす"),
        ];
        let right: &[(&str, &str)] = &[
            ("", "立体"),
            ("Q E", "回転"),
            ("Z X", "仰角"),
            ("", "編集"),
            ("M", "観察と編集"),
            ("T", "道具を換える"),
            ("P", "地形の型"),
            ("R", "別の世界"),
            ("ホイール", "収容力"),
            ("", "共有"),
            ("seed", "同じ世界を開く"),
            ("scenario", "地形の種類"),
        ];
        self.help_column(painter, x, y + 68.0, left);
        self.help_column(painter, x + 480.0, y + 68.0, right);
    }

    pub(crate) fn draw_title(&self, painter: &mut Painter) {
        let (sw, sh) = screen_px();
        painter.rect(0.0, 0.0, sw, sh, UI_PAGE);
        let cx = sw * 0.5;
        let title_px = 64.0;
        let title_y = sh * 0.34;
        self.tx_centered(painter, cx, title_y, title_px, UI_INK, "大陸シミュレーター");
        let rule_w = 88.0;
        let rule_y = title_y + title_px + 20.0;
        painter.rect(cx - rule_w * 0.5, rule_y, rule_w, 1.0, UI_GOLD);
        self.tx_centered(
            painter,
            cx,
            rule_y + 28.0,
            16.0,
            UI_DIM,
            "文明が岸に根を下ろし、国になっていく",
        );
        let pulse = 0.70 + 0.30 * (self.elapsed_secs * 1.5).sin();
        let start = [UI_GOLD[0], UI_GOLD[1], UI_GOLD[2], pulse];
        self.tx_centered(painter, cx, rule_y + 86.0, 22.0, start, "はじめる");
        self.tx_centered(painter, cx, rule_y + 118.0, 12.0, UI_DIM, "Space");
    }

    pub(crate) fn draw_setup(&self, painter: &mut Painter) {
        let (sw, sh) = screen_px();
        painter.rect(0.0, 0.0, sw, sh, UI_PAGE);
        let x = 64.0;
        let mut y = 52.0;
        self.tx(painter, x, y, 40.0, UI_INK, "世界を選ぶ");
        y += 52.0;
        painter.rect(x, y, 56.0, 1.0, UI_GOLD);
        y += 20.0;
        self.tx(painter, x, y, 13.0, UI_DIM, "1 から 8、または左右。Space ではじめる");
        y += 36.0;

        for (i, scenario) in SCENARIOS.iter().enumerate() {
            let sel = i == self.setup_scenario_idx;
            let name = if self.font.is_some() {
                scenario.label_jp()
            } else {
                scenario.label()
            };
            let px = if sel { 20.0 } else { 16.0 };
            self.tx(painter, x, y + if sel { 3.0 } else { 1.0 }, 12.0, UI_DIM, &format!("{}", i + 1));
            self.tx(painter, x + 28.0, y, px, if sel { UI_GOLD } else { UI_DIM }, name);
            y += if sel { 32.0 } else { 26.0 };
        }
        y += 10.0;
        self.tx(
            painter,
            x,
            y,
            15.0,
            UI_INK,
            SCENARIOS[self.setup_scenario_idx].blurb_jp(),
        );
        y += 36.0;

        let styles = [ArtStyle::Pixel, ArtStyle::Diorama, ArtStyle::Solid3D];
        let mut ax = x;
        for style in styles {
            let name = if self.font.is_some() {
                style.label_jp()
            } else {
                style.label()
            };
            let on = style == self.art_style;
            self.tx(painter, ax, y, 16.0, if on { UI_GOLD } else { UI_DIM }, name);
            ax += self.tx_w(painter, 16.0, name) + 22.0;
        }
        y += 44.0;
        self.tx(painter, x, y, 20.0, UI_GOLD, "この世界ではじめる");

        let cell = 4.0;
        let preview_w = COLS as f32 * cell;
        let preview_h = ROWS as f32 * cell;
        let ox = sw - 56.0 - preview_w;
        let oy = (sh - preview_h) * 0.5;
        if self.tiles.len() == COLS * ROWS {
            for row in 0..ROWS {
                for col in 0..COLS {
                    let c = self.tiles[idx(col, row)].terrain.base_color();
                    painter.rect(
                        ox + col as f32 * cell,
                        oy + row as f32 * cell,
                        cell,
                        cell,
                        c,
                    );
                }
            }
        }
    }

    // ---- ship positions (render-derived, no sim mutation) ------------------

    /// Returns a list of `(world_x, world_z, heading_rad)` positions for
    /// animated ships travelling between pairs of coastal settlements.
    ///
    /// Rules (deterministic, no RNG, no sim mutation):
    ///   - Only settlements on tiles adjacent (NEIGHBORS8) to Ocean/DeepOcean
    ///     count as coastal ports.
    ///   - For every pair (a, b) of coastal settlements whose tile-centre
    ///     Euclidean distance is ≤ MAX_SHIP_ROUTE_DIST tiles, one ship travels
    ///     back-and-forth on a straight-line lerp.
    ///   - t = triangle-wave of (elapsed_secs * SHIP_SPEED + per-route phase)
    ///     gives a smooth [0..1] ping-pong along the route.
    ///   - Routes are deduplicated (a < b index order) and capped at MAX_SHIPS.
    pub(crate) fn ship_positions(&self) -> Vec<(f32, f32, f32)> {
        const MAX_SHIP_ROUTE_DIST: f32 = 20.0; // tile-units, Euclidean
        const MAX_SHIPS: usize = 24;
        const SHIP_SPEED: f32 = 0.12; // full lerp cycles per second
        const ROUTE_PHASE_STEP: f32 = 0.37; // irrational offset per route

        // Collect coastal settlement tile world-space centres (world_x, world_z).
        // "Coastal" = at least one NEIGHBORS8 neighbour is Ocean or DeepOcean.
        let coastal: Vec<(usize, f32, f32)> = self
            .settlements
            .iter()
            .enumerate()
            .filter_map(|(si, s)| {
                let col = (s.tile % COLS) as i32;
                let row = (s.tile / COLS) as i32;
                let is_coastal = NEIGHBORS8.iter().any(|&(dc, dr)| {
                    let nc = col + dc;
                    let nr = row + dr;
                    if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                        return false;
                    }
                    matches!(
                        self.tiles[idx(nc as usize, nr as usize)].terrain,
                        Terrain::Ocean | Terrain::DeepOcean
                    )
                });
                if is_coastal {
                    // World-space tile centre (matches 3D coordinate layout)
                    let wx = (s.tile % COLS) as f32 + 0.5;
                    let wz = (s.tile / COLS) as f32 + 0.5;
                    Some((si, wx, wz))
                } else {
                    None
                }
            })
            .collect();

        let mut ships: Vec<(f32, f32, f32)> = Vec::new();
        let mut route_idx: u32 = 0;

        'outer: for a in 0..coastal.len() {
            for b in (a + 1)..coastal.len() {
                if ships.len() >= MAX_SHIPS {
                    break 'outer;
                }
                let (_, ax, az) = coastal[a];
                let (_, bx, bz) = coastal[b];
                let dx = bx - ax;
                let dz = bz - az;
                let dist = (dx * dx + dz * dz).sqrt();
                if dist > MAX_SHIP_ROUTE_DIST {
                    continue;
                }

                // Per-route deterministic phase offset so ships don't clump.
                let phase = route_idx as f32 * ROUTE_PHASE_STEP;
                route_idx += 1;

                // Triangle wave: t oscillates [0..1..0..1..] smoothly.
                let raw = (self.elapsed_secs * SHIP_SPEED + phase).abs() % 2.0;
                let t = if raw <= 1.0 { raw } else { 2.0 - raw };

                let wx = ax + dx * t;
                let wz = az + dz * t;

                // Heading: angle of travel (forward = towards B when t is rising).
                let heading = dz.atan2(dx); // atan2 in XZ plane

                ships.push((wx, wz, heading));
            }
        }

        ships
    }

    // ---- 3D scene builder --------------------------------------------------

    /// Top of a land neighbour. Water and the map edge stay open so coasts read as cliffs.
    fn neighbor_land_top(&self, col: i32, row: i32, height_scale: f32) -> Option<f32> {
        if col < 0 || row < 0 || col >= COLS as i32 || row >= ROWS as i32 {
            return None;
        }
        let t = self.tiles[idx(col as usize, row as usize)].terrain;
        if t.is_water() {
            return None;
        }
        Some(Self::terrain_elevation_3d(t) * height_scale)
    }

    /// Returns the elevation (world-Y top of block) for the given terrain type.
    fn terrain_elevation_3d(t: Terrain) -> f32 {
        match t {
            Terrain::DeepOcean => 0.00,
            Terrain::Ocean     => 0.12,
            Terrain::River     => 0.25,
            Terrain::Desert    => 0.40,
            Terrain::Tundra    => 0.40,
            Terrain::Plains    => 0.42,
            Terrain::Forest    => 0.52,
            Terrain::Hills     => 0.70,
            Terrain::Mountain  => 1.00,
        }
    }

    /// Computes the full display color of tile `i` in 3D — reuses the same
    /// color logic as the 2D map (terrain base + overlays + nation tint).
    fn tile_color_3d(&self, i: usize, nation_map: &std::collections::HashMap<i32, usize>) -> Color {
        let tile = self.tiles[i];

        // Base terrain color (match 2D fallback path).
        let mut color = tile.terrain.base_color();

        // Mountain snow tint.
        if tile.terrain == Terrain::Mountain {
            let t = ((tile.elevation - MOUNTAIN_LEVEL) / (1.0 - MOUNTAIN_LEVEL)).clamp(0.0, 1.0);
            color = lerp_color(color, [0.92, 0.93, 0.96, 1.0], t * 0.7);
        }

        // Scorch tint (burnt/volcanic).
        if self.scorch[i] > self.year {
            color = lerp_color(color, [0.20, 0.12, 0.08, 1.0], 0.50);
        }

        // Overlay tint – only when not Terrain/Resource so it dims in line with 2D.
        let dim = self.overlay != Overlay::Terrain && self.overlay != Overlay::Resource;
        if dim {
            color = [color[0] * 0.6, color[1] * 0.6, color[2] * 0.6, 1.0];
        }

        // Carrying-capacity overlay.
        match self.overlay {
            Overlay::Carrying => {
                if tile.k > 0.0 {
                    let t = (tile.k / K_MAX).clamp(0.0, 1.0);
                    let ov = [0.10 + 0.20 * t, 0.20 + 0.75 * t, 0.15, 0.55];
                    color = lerp_color(color, [ov[0], ov[1], ov[2], 1.0], ov[3]);
                }
            }
            Overlay::Population => {
                let occ = self.occupied[i];
                if occ >= 0 {
                    let s = &self.settlements[occ as usize];
                    let frac = if tile.k > 0.0 { (s.pop / tile.k).clamp(0.0, 1.0) } else { 0.0 };
                    let ov = [0.85, 0.25 + 0.45 * (1.0 - frac), 0.12, 0.55];
                    color = lerp_color(color, [ov[0], ov[1], ov[2], 1.0], ov[3]);
                }
            }
            Overlay::Nation => {
                let occ = self.occupied[i];
                if occ >= 0 {
                    let nid = self.nation_of.get(occ as usize).copied().unwrap_or(-1);
                    if nid >= 0 {
                        if let Some(&ni) = nation_map.get(&nid) {
                            let nc = self.nations[ni].color;
                            color = lerp_color(color, [nc[0], nc[1], nc[2], 1.0], 0.50);
                        }
                    }
                }
            }
            Overlay::Tech => {
                let occ = self.occupied[i];
                if occ >= 0 {
                    let t = self.settlements[occ as usize].tech.count_ones() as f32 / 4.0;
                    let ov = [0.2 + 0.7 * t, 0.2 + 0.7 * t, 0.6 * (1.0 - t), 0.5];
                    color = lerp_color(color, [ov[0], ov[1], ov[2], 1.0], ov[3]);
                }
            }
            Overlay::Resource => {
                if i < self.resources.len() {
                    let ov: Option<Color> = match self.resources[i] {
                        Resource::Iron   => Some([0.60, 0.65, 0.70, 1.0]),
                        Resource::Horses => Some([0.55, 0.35, 0.15, 1.0]),
                        Resource::Gold   => Some([0.90, 0.80, 0.10, 1.0]),
                        Resource::None   => None,
                    };
                    if let Some(rc) = ov {
                        color = lerp_color(color, rc, 0.55);
                    }
                }
            }
            Overlay::Terrain => {}
        }

        // Nation territory tint (matches 2D).
        if self.overlay != Overlay::Carrying && self.overlay != Overlay::Resource {
            let occ = self.occupied[i];
            if occ >= 0 {
                let nid = self.nation_of.get(occ as usize).copied().unwrap_or(-1);
                if nid >= 0 {
                    if let Some(&ni) = nation_map.get(&nid) {
                        let nc = self.nations[ni].color;
                        // A light tint so territory reads without painting over the land.
                        color = lerp_color(color, [nc[0], nc[1], nc[2], 1.0], 0.18);
                    }
                }
            }
        }

        // Disaster epicentre tint.
        if let Some((epi_tile, _kind, epi_year)) = self.last_disaster {
            if i == epi_tile {
                let age = self.year.saturating_sub(epi_year);
                if age < DISASTER_TINT_YEARS {
                    let fade = 1.0 - age as f32 / DISASTER_TINT_YEARS as f32;
                    color = lerp_color(color, [1.0, 0.25, 0.10, 1.0], 0.55 * fade);
                }
            }
        }

        color
    }

    // ---- static/dynamic mesh split -----------------------------------------

    /// Rebuilds the cached static mesh from current sim state and clears
    /// `scene_dirty`.  Called from `update()` whenever `scene_dirty` is true.
    pub(crate) fn rebuild_static_mesh(&mut self) {
        let (verts, idxs) = self.build_static_mesh();
        self.scene_static = Some((verts, idxs));
        self.scene_dirty = false;
    }

    /// Builds the static portion of the 3D scene: terrain blocks (non-water),
    /// buildings, trees, farmland tint+ridges, and road ribbons.
    ///
    /// Water tiles are excluded because their top face is animated (ripple);
    /// they live in the dynamic mesh built every frame.
    ///
    /// Returns `(vertices, indices)`.
    fn build_static_mesh(&self) -> (Vec<Vertex3D>, Vec<u32>) {
        let height_scale = 1.7_f32;
        let tile_w = 1.0_f32;

        const TREES_PER_TILE: usize = 2;
        const MAX_TREE_TOTAL: usize = 400;
        const MAX_BUILDINGS_PER_TILE: usize = 5;
        let bld_budget = self.settlements.len() * MAX_BUILDINGS_PER_TILE;
        let tree_budget = MAX_TREE_TOTAL.min(COLS * ROWS * TREES_PER_TILE);

        // Static mesh excludes water tiles, so roughly (COLS*ROWS - water count)
        // terrain blocks, plus buildings and trees.
        let mut verts: Vec<Vertex3D> = Vec::with_capacity(
            COLS * ROWS * 24 + bld_budget * 48 + tree_budget * 40,
        );
        let mut idxs: Vec<u32> = Vec::with_capacity(
            COLS * ROWS * 36 + bld_budget * 72 + tree_budget * 56,
        );

        // Build a HashMap<nation_id → index into self.nations> once.
        let nation_map: std::collections::HashMap<i32, usize> = self.nations
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id as i32, i))
            .collect();

        // Seats of real peoples, not every hamlet. A one-tile tribe is not a capital skyline.
        let mut capitals = std::collections::HashSet::<i32>::new();
        for nation in &self.nations {
            let mut best: Option<(i32, f32)> = None;
            for &si in &nation.territory {
                let Some(s) = self.settlements.get(si) else { continue };
                let score = s.pop + if s.city { 100_000.0 } else { 0.0 };
                if best.map(|(_, p)| score > p).unwrap_or(true) {
                    best = Some((si as i32, score));
                }
            }
            if let Some((si, _)) = best {
                capitals.insert(si);
            }
        }

        let mut trees_placed: usize = 0;

        for row in 0..ROWS {
            for col in 0..COLS {
                let i = idx(col, row);
                let tile = self.tiles[i];

                // Water tiles go into the dynamic mesh (ripple animation).
                if tile.terrain.is_water() {
                    continue;
                }

                let elev = Self::terrain_elevation_3d(tile.terrain);
                let top_y = elev * height_scale;
                let block_h = top_y.max(0.05);

                let color = self.tile_color_3d(i, &nation_map);

                let cx = col as f32 + 0.5;
                let cy = block_h * 0.5;
                let cz = row as f32 + 0.5;
                let hx = tile_w * 0.5;
                let hy = block_h * 0.5;
                let hz = tile_w * 0.5;

                let faces: [(Vec3, [[f32; 3]; 4]); 6] = [
                    (Vec3::X, [
                        [cx+hx, cy-hy, cz-hz],
                        [cx+hx, cy+hy, cz-hz],
                        [cx+hx, cy+hy, cz+hz],
                        [cx+hx, cy-hy, cz+hz],
                    ]),
                    (-Vec3::X, [
                        [cx-hx, cy-hy, cz+hz],
                        [cx-hx, cy+hy, cz+hz],
                        [cx-hx, cy+hy, cz-hz],
                        [cx-hx, cy-hy, cz-hz],
                    ]),
                    (Vec3::Y, [
                        [cx-hx, cy+hy, cz+hz],
                        [cx+hx, cy+hy, cz+hz],
                        [cx+hx, cy+hy, cz-hz],
                        [cx-hx, cy+hy, cz-hz],
                    ]),
                    (-Vec3::Y, [
                        [cx-hx, cy-hy, cz-hz],
                        [cx+hx, cy-hy, cz-hz],
                        [cx+hx, cy-hy, cz+hz],
                        [cx-hx, cy-hy, cz+hz],
                    ]),
                    (Vec3::Z, [
                        [cx-hx, cy-hy, cz+hz],
                        [cx+hx, cy-hy, cz+hz],
                        [cx+hx, cy+hy, cz+hz],
                        [cx-hx, cy+hy, cz+hz],
                    ]),
                    (-Vec3::Z, [
                        [cx+hx, cy-hy, cz-hz],
                        [cx-hx, cy-hy, cz-hz],
                        [cx-hx, cy+hy, cz-hz],
                        [cx+hx, cy+hy, cz-hz],
                    ]),
                ];

                // Hoist farmland_stage(i) so it is computed only once per tile.
                let farmland = self.farmland_stage(i);
                let farmland_top_color: Option<Color> = farmland.map(|stage| {
                    let fc = Self::farmland_stage_color(stage);
                    lerp_color(color, [fc[0], fc[1], fc[2], 1.0], 0.55)
                });

                for (face_idx, (normal, corners)) in faces.iter().enumerate() {
                    // Drop the bottom and any side a taller-or-equal land neighbour already covers.
                    let hidden = match face_idx {
                        3 => true,
                        0 => self.neighbor_land_top(col as i32 + 1, row as i32, height_scale)
                            .is_some_and(|ny| ny + 0.001 >= top_y),
                        1 => self.neighbor_land_top(col as i32 - 1, row as i32, height_scale)
                            .is_some_and(|ny| ny + 0.001 >= top_y),
                        4 => self.neighbor_land_top(col as i32, row as i32 + 1, height_scale)
                            .is_some_and(|ny| ny + 0.001 >= top_y),
                        5 => self.neighbor_land_top(col as i32, row as i32 - 1, height_scale)
                            .is_some_and(|ny| ny + 0.001 >= top_y),
                        _ => false,
                    };
                    if hidden {
                        continue;
                    }
                    let bv = verts.len() as u32;
                    let face_color = if face_idx == 2 {
                        farmland_top_color.unwrap_or(color)
                    } else {
                        color
                    };
                    for pos in corners.iter() {
                        verts.push(Vertex3D {
                            position: *pos,
                            normal: normal.to_array(),
                            color: face_color,
                            uv: [-1.0, -1.0],
                        });
                    }
                    idxs.extend_from_slice(&[bv, bv+1, bv+2, bv, bv+2, bv+3]);
                }

                // Farmland ridges (reuses the already-computed farmland value).
                if let Some(stage) = farmland {
                    let fc = Self::farmland_stage_color(stage);
                    let ridge_color: Color = [fc[0] * 0.75, fc[1] * 0.75, fc[2] * 0.75, 1.0];
                    const RIDGE_H: f32 = 0.018;
                    const RIDGE_BIAS: f32 = 0.008;
                    const RIDGE_HALF_D: f32 = 0.05;
                    let ridge_y = top_y + RIDGE_BIAS;
                    let ridge_norm = [0.0_f32, 1.0, 0.0];
                    for &frac_z in &[0.28_f32, 0.62_f32] {
                        let ridge_z_centre = (row as f32) + frac_z;
                        let v0 = [cx - hx, ridge_y,            ridge_z_centre - RIDGE_HALF_D];
                        let v1 = [cx + hx, ridge_y,            ridge_z_centre - RIDGE_HALF_D];
                        let v2 = [cx + hx, ridge_y + RIDGE_H,  ridge_z_centre + RIDGE_HALF_D];
                        let v3 = [cx - hx, ridge_y + RIDGE_H,  ridge_z_centre + RIDGE_HALF_D];
                        let bv = verts.len() as u32;
                        for pos in &[v0, v1, v2, v3] {
                            verts.push(Vertex3D {
                                position: *pos,
                                normal: ridge_norm,
                                color: ridge_color,
                                uv: [-1.0, -1.0],
                            });
                        }
                        idxs.extend_from_slice(&[bv, bv+2, bv+1, bv, bv+3, bv+2]);
                    }
                }

                // Forest trees.
                if tile.terrain == Terrain::Forest
                    && self.occupied[i] < 0
                    && trees_placed < MAX_TREE_TOTAL
                {
                    const TRUNK_COLOR: Color  = [0.35, 0.22, 0.12, 1.0];
                    const FOLIAGE_COLOR: Color = [0.18, 0.52, 0.18, 1.0];
                    let trunk_hw   = 0.05_f32;
                    let trunk_h    = 0.12_f32;
                    let foliage_r  = 0.20_f32;
                    let foliage_h  = 0.28_f32;
                    const TREE_OFFSETS: [(f32, f32); 2] = [(-0.22, 0.15), (0.22, -0.15)];
                    let phase_x = if (i / 3) % 2 == 0 { 1.0_f32 } else { -1.0 };
                    let phase_z = if (i / 7) % 2 == 0 { 1.0_f32 } else { -1.0 };
                    for t_idx in 0..TREES_PER_TILE {
                        if trees_placed >= MAX_TREE_TOTAL { break; }
                        let (ox, oz) = TREE_OFFSETS[t_idx];
                        let tx = cx + ox * phase_x;
                        let tz = cz + oz * phase_z;
                        append_box_mesh(&mut verts, &mut idxs,
                            tx, top_y + trunk_h * 0.5, tz,
                            trunk_hw, trunk_h * 0.5, trunk_hw, TRUNK_COLOR);
                        append_pyramid_mesh(&mut verts, &mut idxs,
                            tx, top_y + trunk_h, tz,
                            foliage_r, foliage_h, FOLIAGE_COLOR);
                        trees_placed += 1;
                    }
                }

                // Settlements / buildings.
                let occ = self.occupied[i];
                if occ >= 0 && self.overlay != Overlay::Population {
                    let s = &self.settlements[occ as usize];
                    let major = s.city || (capitals.contains(&occ) && s.pop >= 70.0);
                    // Hamlets stay as land. Villages are one hut. Cities and great seats cluster.
                    if !major && s.pop < 45.0 {
                        continue;
                    }
                    let nid = self.nation_of.get(occ as usize).copied().unwrap_or(-1);

                    // Look up the owning nation's era from its tech bit-count.
                    // Era index: 0=Stone/primitive, 1=Neolithic, 2=Ancient,
                    //            3=Medieval, 4=Modern/Renaissance-Industrial.
                    let era_idx: u32 = if nid >= 0 {
                        if let Some(&ni) = nation_map.get(&nid) {
                            (self.nations[ni].tech.count_ones()).min(4)
                        } else {
                            0
                        }
                    } else {
                        0
                    };

                    let roof_color: Color = if nid >= 0 {
                        if let Some(&ni) = nation_map.get(&nid) {
                            let nc = self.nations[ni].color;
                            [nc[0] * 0.80 + 0.12, nc[1] * 0.70 + 0.06, nc[2] * 0.70 + 0.05, 1.0]
                        } else {
                            [0.62, 0.38, 0.28, 1.0]
                        }
                    } else {
                        [0.62, 0.38, 0.28, 1.0]
                    };

                    // Era-specific building parameters.
                    // era0/Stone:    small low huts, earthy wall color, flat/minimal roof
                    // era1/Neolithic: slightly larger, same earthy tones
                    // era2/Ancient:  standard house (original baseline)
                    // era3/Medieval: taller, stone wall color
                    // era4/Modern:   larger footprint, taller, varied
                    let (wall_color, house_hw_base, house_hd_base, wall_h_base, roof_rise_base):
                        (Color, f32, f32, f32, f32) = match era_idx {
                        0 => ([0.72, 0.60, 0.42, 1.0], 0.14, 0.11, 0.10, 0.04), // tiny mud hut, barely any roof
                        1 => ([0.78, 0.68, 0.50, 1.0], 0.17, 0.13, 0.13, 0.07), // slightly larger, thatch
                        2 => ([0.88, 0.84, 0.74, 1.0], 0.22, 0.16, 0.18, 0.12), // original (ancient/classic)
                        3 => ([0.72, 0.70, 0.68, 1.0], 0.24, 0.18, 0.24, 0.14), // stone tones, taller
                        _ => ([0.82, 0.80, 0.76, 1.0], 0.27, 0.20, 0.28, 0.16), // larger, steeper roof
                    };

                    // hamlet<village<town<city scaling applied on top of era base.
                    let size_scale: f32 = if s.city { 1.20 }
                        else if s.pop < 50.0  { 0.80 }
                        else if s.pop < 200.0 { 0.95 }
                        else { 1.05 };
                    let house_hw  = house_hw_base  * size_scale;
                    let house_hd  = house_hd_base  * size_scale;
                    let wall_h    = wall_h_base    * size_scale;
                    let roof_rise = roof_rise_base * size_scale;

                    let house_count: usize = if major {
                        if s.pop >= 180.0 { 3 } else { 2 }
                    } else {
                        1
                    };

                    // Deterministic jitter: tile-index based, no rng.
                    // Extended jitter table to avoid repetition across many buildings.
                    const JITTER: [(f32, f32); 4] = [
                        (-0.20, 0.18), (0.20, -0.18), (-0.20, -0.20), (0.22, 0.20),
                    ];
                    let phase  = i % 4;
                    let sign_x = if (i / 4) % 2 == 0 { 1.0_f32 } else { -1.0 };
                    let sign_z = if (i / 8) % 2 == 0 { 1.0_f32 } else { -1.0 };

                    // Per-building height jitter (tile-hash based, deterministic).
                    // Gives small variation without RNG.
                    let h_jitter_scale = (i.wrapping_mul(0x9e3779b9) >> 28) as f32 / 30.0; // 0..~0.53

                    // Clamp per-building jitter so the footprint stays within the tile.
                    // |jitter| + house_hw must not exceed 0.46 (margin under 0.5 half-tile).
                    let max_jit = (0.46 - house_hw).max(0.0);

                    for h_idx in 0..house_count {
                        let (jx_raw, jz_raw) = JITTER[(h_idx + phase) % 4];
                        // Scale the deterministic offset down when the building is large.
                        let jitter_mag = jx_raw.abs().max(jz_raw.abs());
                        let scale = if jitter_mag > max_jit && jitter_mag > 0.0 {
                            max_jit / jitter_mag
                        } else {
                            1.0
                        };
                        let jx = jx_raw * scale;
                        let jz = jz_raw * scale;
                        let bx = cx + jx * sign_x;
                        let bz = cz + jz * sign_z;
                        // Small per-house height variation (era0 stays very flat).
                        let h_var = if era_idx == 0 { 0.0 } else {
                            h_jitter_scale * 0.04 * (h_idx as f32 + 1.0)
                        };
                        let this_wall_h    = wall_h + h_var;
                        let this_roof_rise = if era_idx == 0 {
                            roof_rise // flat/minimal — no extra rise
                        } else {
                            roof_rise + h_var * 0.5
                        };
                        append_box_mesh(&mut verts, &mut idxs,
                            bx, top_y + this_wall_h * 0.5, bz,
                            house_hw, this_wall_h * 0.5, house_hd, wall_color);
                        append_roof_mesh(&mut verts, &mut idxs,
                            bx, top_y + this_wall_h, bz,
                            house_hw, house_hd, this_roof_rise, roof_color);
                    }

                    let draw_tower = major;
                    if draw_tower {
                        // Tower dimensions and colours vary by era.
                        let (tower_hw, tower_h, cap_h, tower_col, cap_col): (f32, f32, f32, Color, Color) =
                            match era_idx {
                            0 | 1 => (
                                0.10, 0.30, 0.06,
                                [0.65, 0.55, 0.42, 1.0], // wood/mud watchtower
                                [0.45, 0.35, 0.25, 1.0],
                            ),
                            2 => (
                                0.14, 0.52, 0.08,
                                [0.55, 0.55, 0.62, 1.0], // original stone keep
                                [0.25, 0.22, 0.20, 1.0],
                            ),
                            3 => (
                                0.13, 0.68, 0.10,
                                [0.62, 0.60, 0.65, 1.0], // taller medieval keep
                                [0.30, 0.28, 0.26, 1.0],
                            ),
                            _ => (
                                0.15, 0.80, 0.10,
                                [0.68, 0.66, 0.70, 1.0], // renaissance/industrial tower
                                [0.28, 0.26, 0.30, 1.0],
                            ),
                        };
                        // Second tower offset for era3+/cities: placed slightly off-centre.
                        let tower_x = if era_idx >= 3 && s.city {
                            cx + 0.10 * sign_x
                        } else {
                            cx
                        };
                        let tower_z = if era_idx >= 3 && s.city {
                            cz + 0.10 * sign_z
                        } else {
                            cz
                        };
                        append_box_mesh(&mut verts, &mut idxs,
                            tower_x, top_y + tower_h * 0.5, tower_z,
                            tower_hw, tower_h * 0.5, tower_hw, tower_col);
                        append_box_mesh(&mut verts, &mut idxs,
                            tower_x, top_y + tower_h + cap_h * 0.5, tower_z,
                            tower_hw + 0.03, cap_h * 0.5, tower_hw + 0.03, cap_col);

                        // Additional flanking tower for era4 cities (keeps within footprint).
                        if era_idx >= 4 && s.city {
                            let t2x = cx - 0.18 * sign_x;
                            let t2z = cz - 0.12 * sign_z;
                            let t2_hw = tower_hw * 0.70;
                            let t2_h  = tower_h  * 0.75;
                            append_box_mesh(&mut verts, &mut idxs,
                                t2x, top_y + t2_h * 0.5, t2z,
                                t2_hw, t2_h * 0.5, t2_hw, tower_col);
                            append_box_mesh(&mut verts, &mut idxs,
                                t2x, top_y + t2_h + cap_h * 0.5, t2z,
                                t2_hw + 0.02, cap_h * 0.5, t2_hw + 0.02, cap_col);
                        }
                    }
                }
            }
        }

        // Road ribbons.
        {
            const ROAD_COLOR_3D: Color = [0.42, 0.27, 0.12, 1.0];
            const ROAD_HALF_WIDTH: f32 = 0.12;
            const ROAD_Y_BIAS: f32 = 0.015;
            for (tile_a, tile_b) in self.road_segments() {
                let col_a = tile_a % COLS;
                let row_a = tile_a / COLS;
                let col_b = tile_b % COLS;
                let row_b = tile_b / COLS;
                let ax = col_a as f32 + 0.5;
                let az = row_a as f32 + 0.5;
                let bx = col_b as f32 + 0.5;
                let bz = row_b as f32 + 0.5;
                let elev_a = Self::terrain_elevation_3d(self.tiles[tile_a].terrain) * height_scale;
                let elev_b = Self::terrain_elevation_3d(self.tiles[tile_b].terrain) * height_scale;
                let road_y = elev_a.max(elev_b) + ROAD_Y_BIAS;
                let dx = bx - ax;
                let dz = bz - az;
                let len = (dx * dx + dz * dz).sqrt().max(1e-4);
                let px = (-dz / len) * ROAD_HALF_WIDTH;
                let pz = ( dx / len) * ROAD_HALF_WIDTH;
                let v0 = [ax - px, road_y, az - pz];
                let v1 = [ax + px, road_y, az + pz];
                let v2 = [bx + px, road_y, bz + pz];
                let v3 = [bx - px, road_y, bz - pz];
                let normal = [0.0_f32, 1.0, 0.0];
                let bv = verts.len() as u32;
                for pos in &[v0, v1, v2, v3] {
                    verts.push(Vertex3D {
                        position: *pos,
                        normal,
                        color: ROAD_COLOR_3D,
                        uv: [-1.0, -1.0],
                    });
                }
                idxs.extend_from_slice(&[bv, bv+1, bv+2, bv, bv+2, bv+3]);
            }
        }

        (verts, idxs)
    }

    /// Builds the dynamic portion of the 3D scene: water tile blocks (with
    /// animated ripple on the top face) and ships.
    ///
    /// The index base (`base_v`) is the number of vertices already present in
    /// the combined buffer (i.e., the length of the static vertex slice), so
    /// indices emitted here correctly reference vertices appended after the static
    /// block.
    fn build_dynamic_mesh(&self, base_v: u32) -> (Vec<Vertex3D>, Vec<u32>) {
        let height_scale = 1.7_f32;
        let tile_w = 1.0_f32;

        // nation_map needed for tile_color_3d (nation tint on water is rare but
        // must be correct to match the old full-rebuild path).
        let nation_map: std::collections::HashMap<i32, usize> = self.nations
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id as i32, i))
            .collect();

        // Each water tile: 6 faces × 4 verts = 24 verts / 36 indices.
        let water_tiles = self.tiles.iter().filter(|t| t.terrain.is_water()).count();
        let mut verts: Vec<Vertex3D> = Vec::with_capacity(water_tiles * 24 + 64 * 24);
        let mut idxs:  Vec<u32>     = Vec::with_capacity(water_tiles * 36 + 64 * 36);

        for row in 0..ROWS {
            for col in 0..COLS {
                let i = idx(col, row);
                let tile = self.tiles[i];
                if !tile.terrain.is_water() {
                    continue;
                }

                let elev = Self::terrain_elevation_3d(tile.terrain);
                let top_y = elev * height_scale;
                let block_h = top_y.max(0.05);
                let color = self.tile_color_3d(i, &nation_map);

                let cx = col as f32 + 0.5;
                let cy = block_h * 0.5;
                let cz = row as f32 + 0.5;
                let hx = tile_w * 0.5;
                let hy = block_h * 0.5;
                let hz = tile_w * 0.5;

                let mut faces: [(Vec3, [[f32; 3]; 4]); 6] = [
                    (Vec3::X, [
                        [cx+hx, cy-hy, cz-hz],
                        [cx+hx, cy+hy, cz-hz],
                        [cx+hx, cy+hy, cz+hz],
                        [cx+hx, cy-hy, cz+hz],
                    ]),
                    (-Vec3::X, [
                        [cx-hx, cy-hy, cz+hz],
                        [cx-hx, cy+hy, cz+hz],
                        [cx-hx, cy+hy, cz-hz],
                        [cx-hx, cy-hy, cz-hz],
                    ]),
                    (Vec3::Y, [
                        [cx-hx, cy+hy, cz+hz],
                        [cx+hx, cy+hy, cz+hz],
                        [cx+hx, cy+hy, cz-hz],
                        [cx-hx, cy+hy, cz-hz],
                    ]),
                    (-Vec3::Y, [
                        [cx-hx, cy-hy, cz-hz],
                        [cx+hx, cy-hy, cz-hz],
                        [cx+hx, cy-hy, cz+hz],
                        [cx-hx, cy-hy, cz+hz],
                    ]),
                    (Vec3::Z, [
                        [cx-hx, cy-hy, cz+hz],
                        [cx+hx, cy-hy, cz+hz],
                        [cx+hx, cy+hy, cz+hz],
                        [cx-hx, cy+hy, cz+hz],
                    ]),
                    (-Vec3::Z, [
                        [cx+hx, cy-hy, cz-hz],
                        [cx-hx, cy-hy, cz-hz],
                        [cx-hx, cy+hy, cz-hz],
                        [cx+hx, cy+hy, cz-hz],
                    ]),
                ];

                // Water ripple on top face.
                {
                    const RIPPLE_AMP: f32   = 0.04;
                    const RIPPLE_SPEED: f32 = 1.2;
                    let tile_phase = (col as f32 * 0.97 + row as f32 * 1.31) % std::f32::consts::TAU;
                    let wave_y = RIPPLE_AMP * (self.elapsed_secs * RIPPLE_SPEED + tile_phase).sin();
                    for v in faces[2].1.iter_mut() {
                        v[1] += wave_y;
                    }
                }

                for (face_idx, (normal, corners)) in faces.iter().enumerate() {
                    // Water is a sheet: keep the top, and only the sides that meet land.
                    if face_idx == 3 {
                        continue;
                    }
                    if face_idx != 2 {
                        let (dc, dr) = match face_idx {
                            0 => (1, 0),
                            1 => (-1, 0),
                            4 => (0, 1),
                            5 => (0, -1),
                            _ => (0, 0),
                        };
                        let nc = col as i32 + dc;
                        let nr = row as i32 + dr;
                        let meets_land = nc >= 0
                            && nr >= 0
                            && nc < COLS as i32
                            && nr < ROWS as i32
                            && !self.tiles[idx(nc as usize, nr as usize)].terrain.is_water();
                        if !meets_land {
                            continue;
                        }
                    }
                    // Index base = static vertex count + already-emitted dynamic verts.
                    let bv = base_v + verts.len() as u32;
                    for pos in corners.iter() {
                        verts.push(Vertex3D {
                            position: *pos,
                            normal: normal.to_array(),
                            color,
                            uv: [-1.0, -1.0],
                        });
                    }
                    idxs.extend_from_slice(&[bv, bv+1, bv+2, bv, bv+2, bv+3]);
                }
            }
        }

        // Ships.
        // Note: append_box_mesh uses `verts.len() as u32` internally as the base for
        // its indices. Since `verts` here is the dynamic-local buffer (starts at 0),
        // each index it emits is relative to the dynamic-local start. After the call we
        // add `base_v` to the newly emitted indices to make them absolute in the combined
        // (static + dynamic) vertex buffer.
        {
            let ocean_top = Self::terrain_elevation_3d(Terrain::Ocean) * height_scale;
            const SHIP_Y_BIAS: f32  = 0.06;
            const HULL_COLOR: Color = [0.38, 0.22, 0.10, 1.0];
            const SAIL_COLOR: Color = [0.94, 0.91, 0.84, 1.0];
            const HULL_HX: f32      = 0.18;
            const HULL_HZ: f32      = 0.07;
            const HULL_HY: f32      = 0.03;
            const MAST_HX: f32      = 0.015;
            const MAST_HY: f32      = 0.12;
            const SAIL_HALF_W: f32  = 0.09;
            const SAIL_H: f32       = 0.10;
            const SAIL_Y_BIAS: f32  = 0.015;

            for (wx, wz, _heading) in self.ship_positions() {
                let hull_cy = ocean_top + SHIP_Y_BIAS + HULL_HY;

                // Hull box.
                {
                    let idx_before = idxs.len();
                    append_box_mesh(&mut verts, &mut idxs, wx, hull_cy, wz, HULL_HX, HULL_HY, HULL_HZ, HULL_COLOR);
                    for ir in &mut idxs[idx_before..] { *ir += base_v; }
                }

                // Mast box.
                let mast_base_y = ocean_top + SHIP_Y_BIAS + HULL_HY * 2.0;
                {
                    let idx_before = idxs.len();
                    append_box_mesh(&mut verts, &mut idxs, wx, mast_base_y + MAST_HY, wz, MAST_HX, MAST_HY, MAST_HX, HULL_COLOR);
                    for ir in &mut idxs[idx_before..] { *ir += base_v; }
                }

                // Sail (two quads for double-sided rendering).
                let sail_y0 = mast_base_y + MAST_HY * 2.0 + SAIL_Y_BIAS;
                let normal = [0.0_f32, 1.0, 0.0];
                {
                    let v0 = [wx - SAIL_HALF_W, sail_y0,          wz];
                    let v1 = [wx + SAIL_HALF_W, sail_y0,          wz];
                    let v2 = [wx + SAIL_HALF_W, sail_y0 + SAIL_H, wz];
                    let v3 = [wx - SAIL_HALF_W, sail_y0 + SAIL_H, wz];
                    let bv = base_v + verts.len() as u32;
                    for pos in &[v0, v1, v2, v3] {
                        verts.push(Vertex3D { position: *pos, normal, color: SAIL_COLOR, uv: [-1.0, -1.0] });
                    }
                    idxs.extend_from_slice(&[bv, bv+1, bv+2, bv, bv+2, bv+3]);
                }
                {
                    let v0 = [wx - SAIL_HALF_W, sail_y0,          wz];
                    let v1 = [wx + SAIL_HALF_W, sail_y0,          wz];
                    let v2 = [wx + SAIL_HALF_W, sail_y0 + SAIL_H, wz];
                    let v3 = [wx - SAIL_HALF_W, sail_y0 + SAIL_H, wz];
                    let bv = base_v + verts.len() as u32;
                    for pos in &[v0, v1, v2, v3] {
                        verts.push(Vertex3D { position: *pos, normal, color: SAIL_COLOR, uv: [-1.0, -1.0] });
                    }
                    idxs.extend_from_slice(&[bv, bv+2, bv+1, bv, bv+3, bv+2]);
                }
            }
        }

        (verts, idxs)
    }

    /// Computes the camera and lighting uniforms from current view/time state.
    fn build_camera_and_sky(&self) -> (CameraUniform, [f32; 4]) {
        let map_cx = COLS as f32 * 0.5;
        let map_cz = ROWS as f32 * 0.5;
        let target = Vec3::new(map_cx + self.view_pan.x, 0.0, map_cz + self.view_pan.y);

        let base_half_w = COLS as f32 * 0.46;
        let base_half_h = ROWS as f32 * 0.46;
        let half_w = base_half_w / self.view_zoom;
        let half_h = base_half_h / self.view_zoom;
        let distance = (COLS.max(ROWS) as f32) * 1.2 / self.view_zoom;

        let view = diorama_view(target, self.view_elevation, self.view_azimuth, distance);
        let proj = proj_ortho(half_w, half_h, 0.1_f32, distance * 4.0);
        let view_proj = (proj * view).to_cols_array_2d();

        const DAY_CYCLE_SECS: f32 = 120.0;
        let day_phase = (self.elapsed_secs / DAY_CYCLE_SECS).fract();
        const MIN_SUN_ELEV: f32 = 0.18;
        let sun_elev_t = (std::f32::consts::PI * 2.0 * day_phase).cos();
        let sun_angle = MIN_SUN_ELEV
            + (std::f32::consts::FRAC_PI_2 - MIN_SUN_ELEV) * ((sun_elev_t + 1.0) * 0.5);
        let sun_az = std::f32::consts::TAU * day_phase + 0.8;
        let light_dir = Vec3::new(
            sun_az.cos() * sun_angle.cos(),
            sun_angle.sin().max(MIN_SUN_ELEV),
            sun_az.sin() * sun_angle.cos(),
        ).normalize();

        const AMBIENT_DAY:   f32 = 0.55;
        const AMBIENT_NIGHT: f32 = 0.34;
        let ambient = AMBIENT_NIGHT + (AMBIENT_DAY - AMBIENT_NIGHT) * ((sun_elev_t + 1.0) * 0.5);

        // Clear matches the zenith of the backdrop. No dusk salmon.
        let sky_color: [f32; 4] = [0.012, 0.016, 0.032, 1.0];

        let camera = CameraUniform {
            view_proj,
            light_dir: light_dir.to_array(),
            ambient,
        };
        (camera, sky_color)
    }

    /// Assembles a `Scene3D` from the cached static mesh + freshly built dynamic
    /// mesh.  Called from `scene_3d()` which takes `&self`.
    ///
    /// If `scene_static` is `None` (e.g. first frame before `update()` ran the
    /// rebuild), falls back to building the static mesh inline so the first frame
    /// is never blank.
    pub(crate) fn build_scene_3d_cached(&self) -> Scene3D {
        // Obtain the static geometry (clone from cache, or build inline as fallback).
        let (static_v, static_i) = match &self.scene_static {
            Some((v, i)) => (v.clone(), i.clone()),
            None => self.build_static_mesh(),
        };

        let base_v = static_v.len() as u32;
        let (dyn_v, dyn_i) = self.build_dynamic_mesh(base_v);

        // Merge static + dynamic into a single flat mesh.
        let mut all_vertices = static_v;
        let mut all_indices  = static_i;
        all_vertices.extend_from_slice(&dyn_v);
        all_indices.extend_from_slice(&dyn_i);
        append_sky_backdrop(&mut all_vertices, &mut all_indices, self.view_azimuth);

        let (camera, sky_color) = self.build_camera_and_sky();

        Scene3D {
            camera,
            meshes: vec![(all_vertices, all_indices)],
            texture: self.atlas,
            sky: Some(sky_color),
        }
    }
}

// ---- mesh helpers (free functions) -----------------------------------------

/// A tall wall on the far side of the view. Vertex color runs from a warm
/// horizon up into deep ink, so the clear color is a sky and not a void.
fn append_sky_backdrop(verts: &mut Vec<Vertex3D>, idxs: &mut Vec<u32>, azimuth: f32) {
    let sin_az = azimuth.sin();
    let cos_az = azimuth.cos();
    let dist = 52.0_f32;
    let cx = COLS as f32 * 0.5 - sin_az * dist;
    let cz = ROWS as f32 * 0.5 - cos_az * dist;
    let rx = cos_az;
    let rz = -sin_az;
    let half = 110.0_f32;
    let bands: [(f32, f32, Color, Color); 3] = [
        (-6.0, 8.0, [0.46, 0.24, 0.08, 1.0], [0.16, 0.09, 0.04, 1.0]),
        (8.0, 28.0, [0.16, 0.09, 0.04, 1.0], [0.04, 0.045, 0.07, 1.0]),
        (28.0, 90.0, [0.04, 0.045, 0.07, 1.0], [0.012, 0.016, 0.032, 1.0]),
    ];
    let up = [0.0_f32, 1.0, 0.0];
    for (y0, y1, c0, c1) in bands {
        let bl = [cx - rx * half, y0, cz - rz * half];
        let br = [cx + rx * half, y0, cz + rz * half];
        let tr = [cx + rx * half, y1, cz + rz * half];
        let tl = [cx - rx * half, y1, cz - rz * half];
        let bv = verts.len() as u32;
        for (pos, color) in [(bl, c0), (br, c0), (tr, c1), (tl, c1)] {
            verts.push(Vertex3D {
                position: pos,
                normal: up,
                color,
                uv: [-1.0, -1.0],
            });
        }
        idxs.extend_from_slice(&[bv, bv + 1, bv + 2, bv, bv + 2, bv + 3]);
        let bv = verts.len() as u32;
        for (pos, color) in [(bl, c0), (br, c0), (tr, c1), (tl, c1)] {
            verts.push(Vertex3D {
                position: pos,
                normal: up,
                color,
                uv: [-1.0, -1.0],
            });
        }
        idxs.extend_from_slice(&[bv, bv + 2, bv + 1, bv, bv + 3, bv + 2]);
    }
}

/// Appends a box mesh centred at (`cx`, `cy`, `cz`) with half-extents
/// (`hx`, `hy`, `hz`) into the shared vertex/index buffers.
///
/// Uses the same CCW outward winding as `cube_mesh` in the engine so
/// back-face culling keeps all faces lit.
fn append_box_mesh(
    verts: &mut Vec<Vertex3D>,
    idxs: &mut Vec<u32>,
    cx: f32, cy: f32, cz: f32,
    hx: f32, hy: f32, hz: f32,
    color: Color,
) {
    let faces: [(Vec3, [[f32; 3]; 4]); 6] = [
        // +X right
        (Vec3::X, [
            [cx+hx, cy-hy, cz-hz],
            [cx+hx, cy+hy, cz-hz],
            [cx+hx, cy+hy, cz+hz],
            [cx+hx, cy-hy, cz+hz],
        ]),
        // -X left
        (-Vec3::X, [
            [cx-hx, cy-hy, cz+hz],
            [cx-hx, cy+hy, cz+hz],
            [cx-hx, cy+hy, cz-hz],
            [cx-hx, cy-hy, cz-hz],
        ]),
        // +Y top
        (Vec3::Y, [
            [cx-hx, cy+hy, cz+hz],
            [cx+hx, cy+hy, cz+hz],
            [cx+hx, cy+hy, cz-hz],
            [cx-hx, cy+hy, cz-hz],
        ]),
        // -Y bottom
        (-Vec3::Y, [
            [cx-hx, cy-hy, cz-hz],
            [cx+hx, cy-hy, cz-hz],
            [cx+hx, cy-hy, cz+hz],
            [cx-hx, cy-hy, cz+hz],
        ]),
        // +Z front
        (Vec3::Z, [
            [cx-hx, cy-hy, cz+hz],
            [cx+hx, cy-hy, cz+hz],
            [cx+hx, cy+hy, cz+hz],
            [cx-hx, cy+hy, cz+hz],
        ]),
        // -Z back
        (-Vec3::Z, [
            [cx+hx, cy-hy, cz-hz],
            [cx-hx, cy-hy, cz-hz],
            [cx-hx, cy+hy, cz-hz],
            [cx+hx, cy+hy, cz-hz],
        ]),
    ];
    for (normal, corners) in &faces {
        let bv = verts.len() as u32;
        for pos in corners {
            // Sentinel uv [-1,-1]: uv.x < 0 tells the shader to skip texture
            // sampling and use pure vertex color (see 3d.wgsl white-sentinel).
            verts.push(Vertex3D { position: *pos, normal: normal.to_array(), color, uv: [-1.0, -1.0] });
        }
        idxs.extend_from_slice(&[bv, bv+1, bv+2, bv, bv+2, bv+3]);
    }
}

/// Appends a triangular-prism pitched roof mesh.
///
/// The base of the roof sits at world-Y = `base_y`, centred at (`cx`, ·, `cz`).
/// `half_w` is the half-width along X (eaves), `half_d` is the half-depth along Z
/// (gable ends), and `rise` is the ridge height above `base_y`.
///
/// Produces 5 faces: 2 sloping roof planes (quads), 2 triangular gable ends,
/// and 1 ridge line is merged into the quads.  All normals are outward CCW.
fn append_roof_mesh(
    verts: &mut Vec<Vertex3D>,
    idxs: &mut Vec<u32>,
    cx: f32, base_y: f32, cz: f32,
    half_w: f32, half_d: f32, rise: f32,
    color: Color,
) {
    // Corner positions.
    // Ridge runs along Z (eave along X).
    let ridge_y = base_y + rise;

    // Eave corners (4): bottom of the two sloping planes.
    let fl = [cx - half_w, base_y,  cz - half_d]; // front-left  eave
    let fr = [cx + half_w, base_y,  cz - half_d]; // front-right eave
    let bl = [cx - half_w, base_y,  cz + half_d]; // back-left  eave
    let br = [cx + half_w, base_y,  cz + half_d]; // back-right eave

    // Ridge corners (2): top centre of the roof.
    let rf = [cx, ridge_y, cz - half_d]; // ridge-front
    let rb = [cx, ridge_y, cz + half_d]; // ridge-back

    // ---- left slope (-X side): bl, fl, rf, rb
    // Outward normal points in -X/+Y direction.
    // Vertex order as listed is CW seen from outside (-X), so we emit indices
    // in reversed winding: [0,2,1, 0,3,2] to get CCW from outside.
    {
        let run = half_w;
        let len = (run * run + rise * rise).sqrt().max(1e-6);
        let nx = -rise / len;   // outward: negative X component
        let ny =  run / len;    // outward: positive Y component
        let normal = [nx, ny, 0.0_f32];
        let bv = verts.len() as u32;
        for pos in &[bl, fl, rf, rb] {
            verts.push(Vertex3D { position: *pos, normal, color, uv: [-1.0, -1.0] });
        }
        // reversed winding: CCW from -X exterior
        idxs.extend_from_slice(&[bv, bv+2, bv+1, bv, bv+3, bv+2]);
    }

    // ---- right slope (+X side): fr, br, rb, rf
    // Outward normal points in +X/+Y direction.
    // Vertex order as listed is CW seen from outside (+X), reversed to CCW.
    {
        let run = half_w;
        let len = (run * run + rise * rise).sqrt().max(1e-6);
        let nx =  rise / len;   // outward: positive X component
        let ny =  run / len;    // outward: positive Y component
        let normal = [nx, ny, 0.0_f32];
        let bv = verts.len() as u32;
        for pos in &[fr, br, rb, rf] {
            verts.push(Vertex3D { position: *pos, normal, color, uv: [-1.0, -1.0] });
        }
        // reversed winding: CCW from +X exterior
        idxs.extend_from_slice(&[bv, bv+2, bv+1, bv, bv+3, bv+2]);
    }

    // ---- front gable (-Z): fl, fr, rf  (triangle, CCW from -Z exterior)
    // Listed order fl→fr→rf is CW from -Z, so reverse to CCW.
    {
        let normal = [0.0_f32, 0.0, -1.0];
        let bv = verts.len() as u32;
        for pos in &[fl, fr, rf] {
            verts.push(Vertex3D { position: *pos, normal, color, uv: [-1.0, -1.0] });
        }
        // reversed winding: CCW from -Z exterior
        idxs.extend_from_slice(&[bv, bv+2, bv+1]);
    }

    // ---- back gable (+Z): br, bl, rb  (triangle, CCW from +Z exterior)
    // Listed order br→bl→rb is CW from +Z, so reverse to CCW.
    {
        let normal = [0.0_f32, 0.0, 1.0];
        let bv = verts.len() as u32;
        for pos in &[br, bl, rb] {
            verts.push(Vertex3D { position: *pos, normal, color, uv: [-1.0, -1.0] });
        }
        // reversed winding: CCW from +Z exterior
        idxs.extend_from_slice(&[bv, bv+2, bv+1]);
    }
}

/// Appends a square-base pyramid mesh for low-poly foliage.
///
/// Base square sits at world-Y = `base_y`, centred at (`cx`, ·, `cz`).
/// `half_r` is the half-width of the square base along X and Z.
/// `height` is the full height from base to apex.
///
/// Produces 4 triangular side faces; the base quad is omitted (it is hidden
/// inside the trunk box).  All side-face normals are outward CCW.
fn append_pyramid_mesh(
    verts: &mut Vec<Vertex3D>,
    idxs:  &mut Vec<u32>,
    cx: f32, base_y: f32, cz: f32,
    half_r: f32, height: f32,
    color: Color,
) {
    // Base corners (CCW order when viewed from above).
    let b_fl = [cx - half_r, base_y, cz - half_r]; // front-left
    let b_fr = [cx + half_r, base_y, cz - half_r]; // front-right
    let b_br = [cx + half_r, base_y, cz + half_r]; // back-right
    let b_bl = [cx - half_r, base_y, cz + half_r]; // back-left
    let apex  = [cx,          base_y + height, cz];  // top

    // Precompute the outward normal for each sloped face.
    // For a square pyramid, each face normal is the average of the edge normals,
    // tilted outward by (half_r, height) in the face's perpendicular plane.
    // Normalised outward slope component: run = half_r, rise = height.
    let slope_len = (half_r * half_r + height * height).sqrt().max(1e-6);
    let n_slope = height / slope_len;   // Y (upward) component
    let n_lat   = half_r / slope_len;   // lateral (outward) component

    // ---- front face (-Z side): b_fl, b_fr, apex  — outward normal is (0, n_slope, -n_lat)
    // Listed CW from -Z exterior → emit reversed for CCW.
    {
        let normal = [0.0_f32, n_slope, -n_lat];
        let bv = verts.len() as u32;
        for pos in &[b_fl, b_fr, apex] {
            verts.push(Vertex3D { position: *pos, normal, color, uv: [-1.0, -1.0] });
        }
        // b_fl(0), b_fr(1), apex(2) is CW from outside → reversed: 0,2,1
        idxs.extend_from_slice(&[bv, bv+2, bv+1]);
    }

    // ---- right face (+X side): b_fr, b_br, apex  — outward normal is (n_lat, n_slope, 0)
    {
        let normal = [n_lat, n_slope, 0.0_f32];
        let bv = verts.len() as u32;
        for pos in &[b_fr, b_br, apex] {
            verts.push(Vertex3D { position: *pos, normal, color, uv: [-1.0, -1.0] });
        }
        // b_fr(0), b_br(1), apex(2) is CW from +X → reversed: 0,2,1
        idxs.extend_from_slice(&[bv, bv+2, bv+1]);
    }

    // ---- back face (+Z side): b_br, b_bl, apex  — outward normal is (0, n_slope, n_lat)
    {
        let normal = [0.0_f32, n_slope, n_lat];
        let bv = verts.len() as u32;
        for pos in &[b_br, b_bl, apex] {
            verts.push(Vertex3D { position: *pos, normal, color, uv: [-1.0, -1.0] });
        }
        // b_br(0), b_bl(1), apex(2) is CW from +Z → reversed: 0,2,1
        idxs.extend_from_slice(&[bv, bv+2, bv+1]);
    }

    // ---- left face (-X side): b_bl, b_fl, apex  — outward normal is (-n_lat, n_slope, 0)
    {
        let normal = [-n_lat, n_slope, 0.0_f32];
        let bv = verts.len() as u32;
        for pos in &[b_bl, b_fl, apex] {
            verts.push(Vertex3D { position: *pos, normal, color, uv: [-1.0, -1.0] });
        }
        // b_bl(0), b_fl(1), apex(2) is CW from -X → reversed: 0,2,1
        idxs.extend_from_slice(&[bv, bv+2, bv+1]);
    }
}
