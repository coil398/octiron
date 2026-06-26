use crate::*;
use octiron::{CameraUniform, Scene3D, Vertex3D, diorama_view, proj_ortho};
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
        if t == Terrain::River {
            const ORTHO: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];
            let mut river_dirs: Vec<usize> = Vec::new();
            for (d, &(dc, dr)) in ORTHO.iter().enumerate() {
                let nc = col as i32 + dc;
                let nr = row as i32 + dr;
                if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                    continue;
                }
                if self.tiles[idx(nc as usize, nr as usize)].terrain == Terrain::River {
                    river_dirs.push(d);
                }
            }
            let is_straight = river_dirs == [0, 1] || river_dirs == [2, 3];
            if river_dirs.len() >= 2 && !is_straight {
                if let Some(&r) = self.lib_rects.get("river-bend") {
                    return Some(r);
                }
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
        if t == Terrain::Ocean {
            for &(dc, dr) in NEIGHBORS8.iter() {
                let nc = col as i32 + dc;
                let nr = row as i32 + dr;
                if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                    continue;
                }
                if !self.tiles[idx(nc as usize, nr as usize)].terrain.is_water() {
                    return self.lib_rects.get("coast-edge").copied();
                }
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
                let tint: Color = if dim { [0.5, 0.5, 0.5, 1.0] } else { [1.0, 1.0, 1.0, 1.0] };
                let (atlas, src) = match self.art_style {
                    ArtStyle::Diorama | ArtStyle::Solid3D => {
                        (self.atlas_diorama, self.terrain_src_diorama[tile.terrain.index()])
                    }
                    ArtStyle::Pixel => match self.variant_rect(col, row) {
                        Some(vr) => (self.atlas_lib, Some(vr)),
                        None => (self.atlas, self.terrain_src[tile.terrain.index()]),
                    },
                };
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

        // §2 nations: tint each occupied tile with the nation's color.
        if self.overlay != Overlay::Carrying && self.overlay != Overlay::Resource {
            for nation in &self.nations {
                for &si in &nation.territory {
                    let tile_idx = self.settlements[si].tile;
                    let r = self.view_tile_rect(tile_idx);
                    if r.x + r.w <= MAP_X || r.x >= map_right || r.y + r.h <= MAP_Y || r.y >= map_bottom {
                        continue;
                    }
                    painter.rect(r.x, r.y, r.w, r.h, nation.color);
                }
                let cr = self.view_tile_rect(nation.capital);
                if cr.x + cr.w > MAP_X && cr.x < map_right && cr.y + cr.h > MAP_Y && cr.y < map_bottom {
                    painter.rect(
                        cr.x + 1.0,
                        cr.y + 1.0,
                        cr.w - 2.0,
                        cr.h - 2.0,
                        [1.0, 1.0, 1.0, 0.55],
                    );
                }
            }
        }

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
            for s in &self.settlements {
                let col = s.tile % COLS;
                let row = s.tile / COLS;
                let (tx, ty, tsize) = view_tile(col, row, pan, zoom);
                if tx + tsize <= MAP_X || tx >= map_right || ty + tsize <= MAP_Y || ty >= map_bottom {
                    continue;
                }
                let k = self.tiles[s.tile].k.max(1.0);
                let frac = (s.pop / k).clamp(0.15, 1.0);
                let cx = tx + tsize * 0.5;
                let cy = ty + tsize * 0.5;
                if s.city {
                    let marker = tsize * (0.55 + 0.50 * frac);
                    painter.rect(cx - marker * 0.5 - 1.0, cy - marker * 0.5 - 1.0, marker + 2.0, marker + 2.0, [1.0, 1.0, 1.0, 0.80]);
                    let city_color = lerp_color([0.30, 0.85, 0.90, 1.0], [0.10, 0.50, 0.80, 1.0], frac);
                    painter.rect(cx - marker * 0.5, cy - marker * 0.5, marker, marker, city_color);
                } else {
                    let marker = tsize * (0.40 + 0.55 * frac);
                    let color = lerp_color([1.0, 0.92, 0.55, 1.0], [0.95, 0.35, 0.18, 1.0], frac);
                    painter.rect(cx - marker * 0.5, cy - marker * 0.5, marker, marker, color);
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

        // §6 map editor: hovered tile highlight.
        if self.edit_mode == EditMode::Edit {
            if let Some(hi) = self.hovered_tile {
                let r = self.view_tile_rect(hi);
                painter.rect(r.x, r.y, r.w, 1.5, [1.0, 1.0, 0.6, 0.90]);
                painter.rect(r.x, r.y + r.h - 1.5, r.w, 1.5, [1.0, 1.0, 0.6, 0.90]);
                painter.rect(r.x, r.y, 1.5, r.h, [1.0, 1.0, 0.6, 0.90]);
                painter.rect(r.x + r.w - 1.5, r.y, 1.5, r.h, [1.0, 1.0, 0.6, 0.90]);
            }
        }
    }

    pub(crate) fn draw_panel(&self, painter: &mut Painter) {
        let ink = [0.85, 0.88, 0.95, 1.0];
        let dim = [0.55, 0.60, 0.70, 1.0];
        let accent = [0.55, 0.85, 1.0, 1.0];
        let x = PANEL_X + 12.0;

        painter.rect(PANEL_X, MAP_Y, PANEL_W, MAP_H, [0.10, 0.11, 0.15, 1.0]);

        let mut y = MAP_Y + 10.0;
        if let Some(font) = self.font.as_ref() {
            painter.text_font(x, y, 0.62 * FONT_GLYPH_H, accent, font, "大陸シム");
        } else {
            painter.text(x, y, 0.62, accent, "CONTINENT");
        }
        y += 28.0;

        let pop_total: f32 = self.settlements.iter().map(|s| s.pop).sum();
        let colonized = if self.habitable_count > 0 {
            100.0 * self.settlements.len() as f32 / self.habitable_count as f32
        } else {
            0.0
        };
        let speed_s = if self.paused {
            "停止中".to_string()
        } else {
            format!("x{}", self.speed)
        };

        let last_shock = if let Some((_, kind, yr)) = self.last_disaster {
            format!("{} {}年", kind.label_jp(), yr)
        } else {
            "-".to_string()
        };

        let tech_str = Continent::tech_display(self.all_tech());

        let top_nation_str = if let Some(top) = self
            .nations
            .iter()
            .max_by_key(|n| n.territory.len())
        {
            top.name.clone()
        } else {
            "-".to_string()
        };

        // §9 wars: summary for panel.
        let wars_str = if self.wars.is_empty() {
            "-".to_string()
        } else {
            if let Some(w) = self.wars.iter().max_by_key(|w| w.fronts.len()) {
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
                format!("{} ({} vs {})", self.wars.len(), na, nb)
            } else {
                self.wars.len().to_string()
            }
        };

        // §4 PARKS: leading nation's trait letters.
        let traits_str = if let Some(top_nation) = self
            .nations
            .iter()
            .max_by_key(|n| n.territory.len())
        {
            traits_display(top_nation.traits)
        } else {
            traits_display(0)
        };

        let era_str = self.current_era().label_jp().to_string();

        let might_str = if let Some(top) = self.nations.iter().max_by(|a, b| {
            let pa = self.nation_total_pop(a);
            let pb = self.nation_total_pop(b);
            pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal)
        }) {
            let pop = self.nation_total_pop(top);
            format!("{} ({})", top.name, pop as u64)
        } else {
            "-".to_string()
        };

        let overlay_label = self.overlay.label_jp();

        let stats: [(&str, String); 14] = [
            ("年",      self.year.to_string()),
            ("時代",    era_str),
            ("集落",    self.settlements.len().to_string()),
            ("人口",    format!("{}", pop_total as u64)),
            ("開拓率",  format!("{:.0}%", colonized)),
            ("国家数",  format!("{}", self.nations.len())),
            ("最大国",  top_nation_str),
            ("最強国",  might_str),
            ("戦争",    wars_str),
            ("災害",    format!("{} ({})", self.disaster_count, last_shock)),
            ("技術",    tech_str),
            ("特性",    traits_str),
            ("表示",    overlay_label.to_string()),
            ("速度",    speed_s),
        ];
        for (label, value) in &stats {
            if let Some(font) = self.font.as_ref() {
                painter.text_font(x, y, 0.40 * FONT_GLYPH_H, dim, font, label);
                painter.text_font(x + 72.0, y, 0.40 * FONT_GLYPH_H, ink, font, value);
            } else {
                painter.text(x, y, 0.40, dim, label);
                painter.text(x + 72.0, y, 0.40, ink, value);
            }
            y += 18.0;
        }

        y += 8.0;
        painter.rect(PANEL_X + 8.0, y, PANEL_W - 16.0, 1.0, [0.30, 0.33, 0.40, 1.0]);
        y += 8.0;

        // ---- §8 population / nation history sparkline -----------------------
        // Graph area: full panel inner width, 38 px tall.
        {
            const GRAPH_H: f32 = 38.0;
            const LABEL_W: f32 = 28.0; // width reserved for label on the left

            let gx = x + LABEL_W;          // graph left edge (after label)
            let gy = y;                    // graph top edge
            let gw = PANEL_W - 16.0 - LABEL_W; // graph drawable width

            // Background
            painter.rect(PANEL_X + 8.0, gy, PANEL_W - 16.0, GRAPH_H, [0.07, 0.08, 0.12, 1.0]);

            // Population label (left of graph, vertically centered)
            if let Some(font) = self.font.as_ref() {
                painter.text_font(x, gy + GRAPH_H * 0.5 - 7.0, 0.34 * FONT_GLYPH_H, dim, font, "人口");
            } else {
                painter.text(x, gy + GRAPH_H * 0.5 - 6.0, 0.34, dim, "POP");
            }

            // Population bars
            let n = self.hist_pop.len();
            if n >= 1 {
                let max_pop = self.hist_pop.iter().cloned().fold(1.0f32, f32::max);
                let bar_w = (gw / n as f32).max(1.0);

                for (i, &pop) in self.hist_pop.iter().enumerate() {
                    let t = if max_pop > 0.0 { (pop / max_pop).clamp(0.0, 1.0) } else { 0.0 };
                    let bh = (GRAPH_H * t).max(1.0);
                    let bx = gx + i as f32 * bar_w;
                    let by = gy + GRAPH_H - bh;
                    painter.rect(bx, by, bar_w.max(1.0), bh, [0.85, 0.60, 0.20, 0.85]);
                }
            }

            // Nation-count line
            let nn = self.hist_nations.len();
            if nn >= 1 {
                let max_nations = self.hist_nations.iter().cloned().fold(1u16, u16::max);
                let bar_w = (gw / nn as f32).max(1.0);

                for (i, &cnt) in self.hist_nations.iter().enumerate() {
                    let t = if max_nations > 0 {
                        cnt as f32 / max_nations as f32
                    } else {
                        0.0
                    };
                    let ny = gy + GRAPH_H - (GRAPH_H * t).clamp(1.0, GRAPH_H);
                    let nx = gx + i as f32 * bar_w;
                    painter.rect(nx, ny, bar_w.max(1.0), 2.0, [0.40, 0.90, 1.00, 0.90]);
                }
            }

            y += GRAPH_H + 6.0;
        }

        painter.rect(PANEL_X + 8.0, y, PANEL_W - 16.0, 1.0, [0.30, 0.33, 0.40, 1.0]);
        y += 8.0;
        if let Some(font) = self.font.as_ref() {
            painter.text_font(x, y, 0.42 * FONT_GLYPH_H, accent, font, "年代記");
        } else {
            painter.text(x, y, 0.42, accent, "CHRONICLE");
        }
        y += 20.0;
        for line in &self.log {
            if let Some(font) = self.font.as_ref() {
                painter.text_font(x, y, 0.34 * FONT_GLYPH_H, ink, font, line);
            } else {
                painter.text(x, y, 0.34, ink, line);
            }
            y += 15.0;
        }
    }

    pub(crate) fn draw_status(&self, painter: &mut Painter) {
        let y = MAP_Y + MAP_H + 8.0;
        let base_hint = "[Space]停止 [1-4]速度 [O]表示 [P]地形 [G]画風 [R]新規 [click]調査";
        let edit_hint = "  [M]モード [T]ツール [LMB]使用 [Scroll]K+/-";
        let cam_hint = if self.art_style == ArtStyle::Solid3D {
            "  [Q/E]方位 [Z/X]仰角"
        } else {
            ""
        };

        // Time-of-day label shown only in Solid3D mode.
        let tod_hint = if self.art_style == ArtStyle::Solid3D {
            const DAY_CYCLE_SECS: f32 = 120.0;
            let phase = (self.elapsed_secs / DAY_CYCLE_SECS).fract();
            let label = if phase < 0.125 || phase >= 0.875 {
                "正午"
            } else if phase < 0.375 {
                "夕刻"
            } else if phase < 0.625 {
                "深夜"
            } else {
                "夜明"
            };
            format!("  [{}]", label)
        } else {
            String::new()
        };

        let help_hint = if self.show_help { "" } else { "  [H]ヘルプ" };

        if let Some(font) = self.font.as_ref() {
            painter.text_font(
                MAP_X,
                y,
                0.40 * FONT_GLYPH_H,
                [0.55, 0.60, 0.70, 1.0],
                font,
                &format!("{}{}{}{}{}  地形:{} 画風:{}  SEED {}", base_hint, edit_hint, cam_hint, tod_hint, help_hint, self.scenario.label_jp(), self.art_style.label_jp(), self.seed),
            );
        } else {
            painter.text(
                MAP_X,
                y,
                0.40,
                [0.55, 0.60, 0.70, 1.0],
                &format!("{}{}{}{}{}  preset:{} art:{}  SEED {}", base_hint, edit_hint, cam_hint, tod_hint, help_hint, self.scenario.label(), self.art_style.label(), self.seed),
            );
        }
    }

    /// Font-aware text helper: dispatches to `text_font` if a font is loaded,
    /// else falls back to the built-in bitmap `text`.
    pub(crate) fn pt(&self, painter: &mut Painter, x: f32, y: f32, scale: f32, color: Color, text: &str) {
        if let Some(font) = self.font.as_ref() {
            painter.text_font(x, y, scale * FONT_GLYPH_H, color, font, text);
        } else {
            painter.text(x, y, scale, color, text);
        }
    }

    /// §8 readability: tile inspector box (left-click in Observe mode).
    pub(crate) fn draw_selection_panel(&self, painter: &mut Painter) {
        let Some(ti) = self.selected_tile else {
            return;
        };
        let col = ti % COLS;
        let row = ti / COLS;

        let (hx, hy, hsize) = view_tile(col, row, self.view_pan, self.view_zoom);
        let hl = [1.0, 0.95, 0.4, 1.0];
        let b = 2.0;
        painter.rect(hx, hy, hsize, b, hl);
        painter.rect(hx, hy + hsize - b, hsize, b, hl);
        painter.rect(hx, hy, b, hsize, hl);
        painter.rect(hx + hsize - b, hy, b, hsize, hl);

        let bw = 252.0;
        let bh = 164.0;
        let bx = MAP_X + 6.0;
        let by = MAP_Y + MAP_H - bh - 6.0;
        painter.rect(bx, by, bw, bh, [0.06, 0.07, 0.11, 0.88]);
        let ink = [0.86, 0.89, 0.96, 1.0];
        let dim = [0.55, 0.60, 0.70, 1.0];
        let acc = [0.6, 0.85, 1.0, 1.0];
        let x = bx + 8.0;
        let mut y = by + 8.0;
        let tile = self.tiles[ti];

        self.pt(painter, x, y, 0.42, acc, "調査");
        y += 16.0;
        self.pt(painter, x, y, 0.34, dim, &format!("座標 {},{} / {}", col, row, tile.terrain.tile_name_jp()));
        y += 14.0;
        self.pt(painter, x, y, 0.34, ink,
            &format!("K {:.0}   {}", tile.k, if tile.habitable { "居住可" } else { "不毛" }),
        );
        y += 14.0;
        if ti < self.resources.len() && self.resources[ti] != Resource::None {
            let res_color = match self.resources[ti] {
                Resource::Iron   => [0.75, 0.80, 0.90, 1.0],
                Resource::Horses => [0.80, 0.65, 0.35, 1.0],
                Resource::Gold   => [1.00, 0.85, 0.20, 1.0],
                Resource::None   => ink,
            };
            self.pt(painter, x, y, 0.34, res_color,
                &format!("資源: {}", self.resources[ti].label_jp()));
            y += 14.0;
        }

        let si = self.occupied[ti];
        if si >= 0 {
            let s = &self.settlements[si as usize];
            let mut techs = String::new();
            if s.tech & TECH_IRRIGATION != 0 { techs.push_str("灌漑 "); }
            if s.tech & TECH_SEAFARING != 0  { techs.push_str("航海 "); }
            if s.tech & TECH_METALLURGY != 0 { techs.push_str("冶金 "); }
            if s.tech & TECH_WRITING != 0    { techs.push_str("文字 "); }
            if techs.is_empty() { techs.push('-'); }
            let settlement_label = if s.city { "都市" } else { "集落" };
            let city_color: Color = if s.city { [0.30, 0.90, 0.95, 1.0] } else { ink };
            let inspired_suffix = if s.inspired_years > 0 { " (鼓舞中)" } else { "" };
            self.pt(painter, x, y, 0.34, city_color, &format!("{} 人口 {:.0}  停滞 {}{}", settlement_label, s.pop, s.stuck, inspired_suffix));
            y += 14.0;
            if s.city {
                self.pt(painter, x, y, 0.34, city_color, "** 都市 **  上限+10%");
                y += 14.0;
            }
            self.pt(painter, x, y, 0.34, ink, &format!("技術 {}", techs.trim()));
            y += 14.0;
            self.pt(painter, x, y, 0.34, ink, &format!("特性 {}", traits_display(s.traits)));
            y += 14.0;
            let nid = self.nation_of.get(si as usize).copied().unwrap_or(-1);
            if nid >= 0 {
                if let Some(n) = self.nations.iter().find(|n| n.id as i32 == nid) {
                    let total_pop = self.nation_total_pop(n);
                    let golden_suffix = if n.golden { " (黄金時代)" } else { "" };
                    self.pt(painter, x, y, 0.34, acc, &format!("{} ({} 集落, 人口 {}){}", n.name, n.territory.len(), total_pop as u64, golden_suffix));
                    y += 14.0;
                    self.pt(painter, x, y, 0.34, ink, &format!("文化 #{} | 交易 {} 国", n.culture, n.trade_partners));
                    y += 14.0;
                    let mut why = String::new();
                    if n.traits & TRAIT_DISCIPLINE != 0        { why.push_str("武力 "); }
                    if n.traits & TRAIT_COMMERCE != 0          { why.push_str("商業 "); }
                    if n.traits & TRAIT_SEAFARING_TRAIT != 0   { why.push_str("海洋 "); }
                    if n.traits & TRAIT_MASONRY != 0           { why.push_str("石工 "); }
                    if n.tech & TECH_WRITING != 0              { why.push_str("識字 "); }
                    if why.is_empty() { why.push_str("農耕"); }
                    self.pt(painter, x, y, 0.32, dim, why.trim());
                }
            } else {
                self.pt(painter, x, y, 0.34, dim, "無国籍");
            }
        } else {
            self.pt(painter, x, y, 0.34, dim, "未開地");
        }
    }

    // ---- help overlay (render-only, no sim state) --------------------------

    /// Draws a semi-transparent HELP overlay listing all controls, grouped by
    /// category.  Activated with [H].  Render-only: reads no sim state and
    /// never calls any mutating method, so determinism is unaffected.
    pub(crate) fn draw_help_overlay(&self, painter: &mut Painter) {
        // Backing rect — covers most of the map area for readability.
        let bx = MAP_X + 8.0;
        let by = MAP_Y + 8.0;
        let bw = MAP_W - 16.0;
        let bh = MAP_H - 16.0;
        painter.rect(bx, by, bw, bh, [0.04, 0.05, 0.10, 0.92]);

        let ink:   Color = [0.88, 0.92, 1.00, 1.0];
        let dim:   Color = [0.55, 0.62, 0.75, 1.0];
        let head:  Color = [0.55, 0.85, 1.00, 1.0];
        let key_c: Color = [1.00, 0.90, 0.40, 1.0];

        let col1 = bx + 12.0;
        let col2 = bx + bw * 0.50;
        let mut ly = by + 14.0;

        // ---- Title ----
        self.pt(painter, col1, ly, 0.52, head, "操作ガイド   [H] 閉じる");
        ly += 24.0;

        // Layout: two columns side by side.
        // Left column controls.
        let left: &[(&str, &str)] = &[
            // TIME
            ("-- 時間 --",           ""),
            ("Space",                "一時停止 / 再開"),
            ("1 / 2 / 3 / 4",        "速度 x1/x2/x3/x4"),
            ("",                     ""),
            // VIEW
            ("-- 表示 --",           ""),
            ("O",                    "重ね表示 (地形/収容力/人口/国家/技術/資源)"),
            ("G",                    "画風 (ピクセル / ジオラマ / 3D)"),
            ("click",                "タイル調査"),
            ("+  /  -",              "ズームイン / アウト"),
            ("矢印キー",             "マップ移動"),
            ("",                     ""),
            // 3D CAMERA
            ("-- 3Dカメラ --",       ""),
            ("Q / E",                "方位回転"),
            ("Z / X",                "仰角変更"),
        ];

        // Right column controls.
        let right: &[(&str, &str)] = &[
            // INTERVENE
            ("-- 介入 --",           ""),
            ("P",                    "地形プリセット"),
            ("R",                    "新世界生成"),
            ("M",                    "モード切替 (観察 / 編集)"),
            ("T  (編集モード)",      "ツール切替 (塗装/災害/恵/鼓舞/建国)"),
            ("左クリック (編集)",    "ツール適用"),
            ("右クリック (編集)",    "逆方向適用"),
            ("スクロール (編集)",    "K値調整"),
            ("",                     ""),
            // SHARE
            ("-- 共有 --",           ""),
            ("?seed=N",              "URL: このワールドを再現"),
            ("?scenario=NAME",       "standard/pangaea/archipelago/highlands/arid/iceage/volcanic/lush"),
            ("",                     ""),
            ("H",                    "このヘルプを閉じる"),
        ];

        let row_h = 17.0;
        let key_w = 130.0; // width allocated for key column

        let mut left_y  = ly;
        let mut right_y = ly;

        for &(key, val) in left {
            if key.is_empty() {
                left_y += row_h * 0.5;
                continue;
            }
            if val.is_empty() {
                // Section heading.
                self.pt(painter, col1, left_y, 0.38, head, key);
            } else {
                self.pt(painter, col1, left_y, 0.36, key_c, key);
                self.pt(painter, col1 + key_w, left_y, 0.36, ink, val);
            }
            left_y += row_h;
        }

        for &(key, val) in right {
            if key.is_empty() {
                right_y += row_h * 0.5;
                continue;
            }
            if val.is_empty() {
                // Section heading.
                self.pt(painter, col2, right_y, 0.38, head, key);
            } else {
                self.pt(painter, col2, right_y, 0.36, key_c, key);
                self.pt(painter, col2 + key_w, right_y, 0.36, ink, val);
            }
            right_y += row_h;
        }

        // Footer dim line.
        let footer_y = by + bh - 18.0;
        self.pt(painter, col1, footer_y, 0.34, dim, "画風: ピクセル/ジオラマ/3D (G)   表示: 地形/収容力/人口/国家/技術/資源 (O)");
    }

    // ---- title / setup screens -----------------------------------------------

    /// Draws the full-screen title card (Phase::Title).
    pub(crate) fn draw_title(&self, painter: &mut Painter) {
        use crate::{MAP_Y, MAP_H, PANEL_W, MARGIN, PANEL_X};
        let sw = PANEL_X + PANEL_W + MARGIN;
        let sh = MAP_Y + MAP_H + 30.0;

        // Background.
        painter.rect(0.0, 0.0, sw, sh, [0.04, 0.05, 0.10, 1.0]);

        let cx = sw * 0.5;
        let title_y  = sh * 0.28;
        let tag_y    = title_y + 60.0;
        let hint_y   = sh * 0.72;

        let title_c: Color = [0.55, 0.85, 1.00, 1.0];
        let tag_c:   Color = [0.68, 0.78, 0.95, 1.0];
        let hint_c:  Color = [0.50, 0.55, 0.68, 1.0];

        // Animated star-field: deterministic dots scattered using tile hash.
        for i in 0u32..120 {
            let hx = i.wrapping_mul(0x9e37_79b9) ^ (i << 13);
            let hy = i.wrapping_mul(0x6c62_272e) ^ (i >> 5);
            let sx = (hx % (sw as u32)) as f32;
            let sy = (hy % (sh as u32)) as f32;
            let phase = (i as f32 * 0.618 + self.elapsed_secs * 0.7) % std::f32::consts::TAU;
            let alpha = 0.15 + 0.25 * (phase.sin() * 0.5 + 0.5);
            painter.rect(sx, sy, 2.0, 2.0, [0.7, 0.8, 1.0, alpha]);
        }

        if let Some(font) = self.font.as_ref() {
            // Title (Japanese): "大陸シミュレーター"
            let title_text = "大陸シミュレーター";
            let title_px = 1.20 * FONT_GLYPH_H;
            let title_w = painter.text_font_width(title_px, font, title_text);
            painter.text_font(cx - title_w * 0.5, title_y, title_px, title_c, font, title_text);

            // Tagline.
            let tag = "手続き生成の文明サンドボックス";
            let tag_px = 0.50 * FONT_GLYPH_H;
            let tag_w = painter.text_font_width(tag_px, font, tag);
            painter.text_font(cx - tag_w * 0.5, tag_y, tag_px, tag_c, font, tag);

            // Prompt (pulse alpha).
            let prompt = "-- スペース または クリック で開始 --";
            let prompt_px = 0.44 * FONT_GLYPH_H;
            let alpha = 0.55 + 0.45 * (self.elapsed_secs * 1.8).sin();
            let prompt_c: Color = [hint_c[0], hint_c[1], hint_c[2], alpha];
            let prompt_w = painter.text_font_width(prompt_px, font, prompt);
            painter.text_font(cx - prompt_w * 0.5, hint_y, prompt_px, prompt_c, font, prompt);
        } else {
            // ASCII fallback.
            let title_text = "CONTINENT SIM";
            let title_scale = 1.20;
            let char_w = 8.0 * title_scale;
            let title_w = title_text.len() as f32 * char_w;
            painter.text(cx - title_w * 0.5, title_y, title_scale, title_c, title_text);

            let tag = "A procedural civilization sandbox";
            let tag_scale = 0.50;
            let tag_w = tag.len() as f32 * 8.0 * tag_scale;
            painter.text(cx - tag_w * 0.5, tag_y, tag_scale, tag_c, tag);

            let prompt = "-- PRESS SPACE or CLICK to continue --";
            let prompt_scale = 0.44;
            let alpha = 0.55 + 0.45 * (self.elapsed_secs * 1.8).sin();
            let prompt_c: Color = [hint_c[0], hint_c[1], hint_c[2], alpha];
            let prompt_w = prompt.len() as f32 * 8.0 * prompt_scale;
            painter.text(cx - prompt_w * 0.5, hint_y, prompt_scale, prompt_c, prompt);
        }
    }

    /// Draws the scenario-selection Setup screen (Phase::Setup).
    pub(crate) fn draw_setup(&self, painter: &mut Painter) {
        use crate::{MAP_Y, MAP_H, PANEL_W, MARGIN, PANEL_X, SCENARIOS};
        let sw = PANEL_X + PANEL_W + MARGIN;
        let sh = MAP_Y + MAP_H + 30.0;

        // Background.
        painter.rect(0.0, 0.0, sw, sh, [0.04, 0.05, 0.10, 1.0]);

        let cx = sw * 0.5;
        let title_c: Color = [0.55, 0.85, 1.00, 1.0];
        let ink:     Color = [0.85, 0.88, 0.95, 1.0];
        let dim:     Color = [0.55, 0.60, 0.70, 1.0];
        let sel_c:   Color = [1.00, 0.90, 0.40, 1.0];

        if let Some(font) = self.font.as_ref() {
            // Heading.
            let head = "シナリオを選んでください";
            let head_px = 0.68 * FONT_GLYPH_H;
            let head_w = painter.text_font_width(head_px, font, head);
            painter.text_font(cx - head_w * 0.5, 40.0, head_px, title_c, font, head);

            // Sub-hint.
            let sub = "[< >] または [1-8] で選択   [G] 画風   [SPACE] 開始";
            let sub_px = 0.38 * FONT_GLYPH_H;
            let sub_w = painter.text_font_width(sub_px, font, sub);
            painter.text_font(cx - sub_w * 0.5, 80.0, sub_px, dim, font, sub);

            // Scenario list (2 columns of 4).
            let col_left  = cx - 240.0;
            let col_right = cx + 20.0;
            let row_start_y = 120.0;
            let row_h = 40.0;

            for (i, scenario) in SCENARIOS.iter().enumerate() {
                let col_x = if i < 4 { col_left } else { col_right };
                let row   = if i < 4 { i } else { i - 4 };
                let y     = row_start_y + row as f32 * row_h;

                let is_sel = i == self.setup_scenario_idx;
                let color  = if is_sel { sel_c } else { ink };
                let scale  = if is_sel { 0.52 } else { 0.42 };
                let px     = scale * FONT_GLYPH_H;
                let badge_px = scale * 0.85 * FONT_GLYPH_H;

                // Number badge.
                let badge = format!("[{}]", i + 1);
                painter.text_font(col_x, y, badge_px, dim, font, &badge);

                // Scenario name (Japanese).
                let name = scenario.label_jp();
                painter.text_font(col_x + 38.0, y, px, color, font, name);

                // Selection arrow.
                if is_sel {
                    painter.text_font(col_x - 18.0, y, px, sel_c, font, "▶");
                }
            }

            // Art style indicator.
            let art_str = format!("画風: {}  (G で切替)", self.art_style.label_jp());
            let art_px = 0.40 * FONT_GLYPH_H;
            let art_w = painter.text_font_width(art_px, font, &art_str);
            painter.text_font(cx - art_w * 0.5, sh - 50.0, art_px, dim, font, &art_str);

            // START prompt at bottom.
            let start = "-- スペース で開始 --";
            let start_px = 0.48 * FONT_GLYPH_H;
            let alpha = 0.55 + 0.45 * (self.elapsed_secs * 1.6).sin();
            let start_c: Color = [0.55, 0.85, 1.00, alpha];
            let start_w = painter.text_font_width(start_px, font, start);
            painter.text_font(cx - start_w * 0.5, sh - 28.0, start_px, start_c, font, start);
        } else {
            // ASCII fallback.
            let head = "CHOOSE SCENARIO";
            let head_scale = 0.68;
            let head_w = head.len() as f32 * 8.0 * head_scale;
            painter.text(cx - head_w * 0.5, 40.0, head_scale, title_c, head);

            let sub = "[< >] or [1-8] to select   [G] art style   [SPACE] start";
            let sub_scale = 0.38;
            let sub_w = sub.len() as f32 * 8.0 * sub_scale;
            painter.text(cx - sub_w * 0.5, 80.0, sub_scale, dim, sub);

            let col_left  = cx - 240.0;
            let col_right = cx + 20.0;
            let row_start_y = 120.0;
            let row_h = 40.0;

            for (i, scenario) in SCENARIOS.iter().enumerate() {
                let col_x = if i < 4 { col_left } else { col_right };
                let row   = if i < 4 { i } else { i - 4 };
                let y     = row_start_y + row as f32 * row_h;
                let is_sel = i == self.setup_scenario_idx;
                let color  = if is_sel { sel_c } else { ink };
                let scale  = if is_sel { 0.52 } else { 0.42 };
                let badge = format!("[{}]", i + 1);
                painter.text(col_x, y, scale * 0.85, dim, &badge);
                painter.text(col_x + 38.0, y, scale, color, scenario.label());
                if is_sel {
                    painter.text(col_x - 16.0, y, scale, sel_c, ">");
                }
            }

            let art_str = format!("Art: {}  (G to cycle)", self.art_style.label());
            let art_scale = 0.40;
            let art_w = art_str.len() as f32 * 8.0 * art_scale;
            painter.text(cx - art_w * 0.5, sh - 50.0, art_scale, dim, &art_str);

            let start = "-- PRESS SPACE to start --";
            let start_scale = 0.48;
            let alpha = 0.55 + 0.45 * (self.elapsed_secs * 1.6).sin();
            let start_c: Color = [0.55, 0.85, 1.00, alpha];
            let start_w = start.len() as f32 * 8.0 * start_scale;
            painter.text(cx - start_w * 0.5, sh - 28.0, start_scale, start_c, start);
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
                        // nc is already [r,g,b,a=0.5]; blend over.
                        color = lerp_color(color, [nc[0], nc[1], nc[2], 1.0], nc[3]);
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
        let height_scale = 4.0_f32;
        let tile_w = 1.0_f32;

        const ATLAS_W: f32 = 454.0;
        const ATLAS_H: f32 = 129.0;
        #[inline(always)]
        fn terrain_uv(px: f32, py: f32, pw: f32, ph: f32) -> [f32; 4] {
            [
                (px + 0.5) / ATLAS_W,
                (py + 0.5) / ATLAS_H,
                (px + pw - 0.5) / ATLAS_W,
                (py + ph - 0.5) / ATLAS_H,
            ]
        }
        let terrain_uv_rects: [[f32; 4]; TERRAIN_COUNT] = [
            terrain_uv(  0.0,  0.0, 64.0, 64.0), // 0 DeepOcean
            terrain_uv(325.0,  0.0, 64.0, 64.0), // 1 Ocean
            terrain_uv(  0.0, 65.0, 64.0, 64.0), // 2 River
            terrain_uv(390.0,  0.0, 64.0, 64.0), // 3 Plains
            terrain_uv(130.0,  0.0, 64.0, 64.0), // 4 Forest
            terrain_uv(195.0,  0.0, 64.0, 64.0), // 5 Hills
            terrain_uv(260.0,  0.0, 64.0, 64.0), // 6 Mountain
            terrain_uv( 65.0,  0.0, 64.0, 64.0), // 7 Desert
            terrain_uv( 65.0, 65.0, 64.0, 64.0), // 8 Tundra
        ];

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

                let tidx = tile.terrain.index();
                let [u0, v0_uv, u1, v1_uv] = terrain_uv_rects[tidx];
                let uc = (u0 + u1) * 0.5;
                let vc = (v0_uv + v1_uv) * 0.5;
                let top_uvs: [[f32; 2]; 4] = [
                    [u0, v1_uv],
                    [u1, v1_uv],
                    [u1, v0_uv],
                    [u0, v0_uv],
                ];

                // Hoist farmland_stage(i) so it is computed only once per tile.
                let farmland = self.farmland_stage(i);
                let farmland_top_color: Option<Color> = farmland.map(|stage| {
                    let fc = Self::farmland_stage_color(stage);
                    lerp_color(color, [fc[0], fc[1], fc[2], 1.0], 0.55)
                });

                for (face_idx, (normal, corners)) in faces.iter().enumerate() {
                    let bv = verts.len() as u32;
                    let face_color = if face_idx == 2 {
                        farmland_top_color.unwrap_or(color)
                    } else {
                        color
                    };
                    for (vi, pos) in corners.iter().enumerate() {
                        let uv = if face_idx == 2 { top_uvs[vi] } else { [uc, vc] };
                        verts.push(Vertex3D {
                            position: *pos,
                            normal: normal.to_array(),
                            color: face_color,
                            uv,
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

                    // House count: driven by settlement size, same logic as before.
                    let house_count: usize = if s.city { 4 }
                        else if s.pop < 50.0  { 1 }
                        else if s.pop < 200.0 { 2 }
                        else { 3 };

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

                    // Tower: present for cities always; for large towns in era>=3;
                    // frequency and height scale with era.
                    let draw_tower = s.city || (era_idx >= 3 && s.pop >= 200.0);
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
        let height_scale = 4.0_f32;
        let tile_w = 1.0_f32;

        const ATLAS_W: f32 = 454.0;
        const ATLAS_H: f32 = 129.0;
        #[inline(always)]
        fn terrain_uv(px: f32, py: f32, pw: f32, ph: f32) -> [f32; 4] {
            [
                (px + 0.5) / ATLAS_W,
                (py + 0.5) / ATLAS_H,
                (px + pw - 0.5) / ATLAS_W,
                (py + ph - 0.5) / ATLAS_H,
            ]
        }
        let terrain_uv_rects: [[f32; 4]; TERRAIN_COUNT] = [
            terrain_uv(  0.0,  0.0, 64.0, 64.0), // 0 DeepOcean
            terrain_uv(325.0,  0.0, 64.0, 64.0), // 1 Ocean
            terrain_uv(  0.0, 65.0, 64.0, 64.0), // 2 River
            terrain_uv(390.0,  0.0, 64.0, 64.0), // 3 Plains
            terrain_uv(130.0,  0.0, 64.0, 64.0), // 4 Forest
            terrain_uv(195.0,  0.0, 64.0, 64.0), // 5 Hills
            terrain_uv(260.0,  0.0, 64.0, 64.0), // 6 Mountain
            terrain_uv( 65.0,  0.0, 64.0, 64.0), // 7 Desert
            terrain_uv( 65.0, 65.0, 64.0, 64.0), // 8 Tundra
        ];

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

                let tidx = tile.terrain.index();
                let [u0, v0_uv, u1, v1_uv] = terrain_uv_rects[tidx];
                let uc = (u0 + u1) * 0.5;
                let vc = (v0_uv + v1_uv) * 0.5;
                let top_uvs: [[f32; 2]; 4] = [
                    [u0, v1_uv], [u1, v1_uv], [u1, v0_uv], [u0, v0_uv],
                ];

                for (face_idx, (normal, corners)) in faces.iter().enumerate() {
                    // Index base = static vertex count + already-emitted dynamic verts.
                    let bv = base_v + verts.len() as u32;
                    for (vi, pos) in corners.iter().enumerate() {
                        let uv = if face_idx == 2 { top_uvs[vi] } else { [uc, vc] };
                        verts.push(Vertex3D {
                            position: *pos,
                            normal: normal.to_array(),
                            color,
                            uv,
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

        const AMBIENT_DAY:   f32 = 0.50;
        const AMBIENT_NIGHT: f32 = 0.12;
        let ambient = AMBIENT_NIGHT + (AMBIENT_DAY - AMBIENT_NIGHT) * ((sun_elev_t + 1.0) * 0.5);

        let sky_noon:     [f32; 4] = [0.45, 0.65, 0.95, 1.0];
        let sky_dusk:     [f32; 4] = [0.85, 0.40, 0.25, 1.0];
        let sky_midnight: [f32; 4] = [0.04, 0.05, 0.16, 1.0];
        let sky_dawn:     [f32; 4] = [0.80, 0.45, 0.20, 1.0];
        let sky_color: [f32; 4] = if day_phase < 0.25 {
            lerp_color(sky_noon, sky_dusk, day_phase / 0.25)
        } else if day_phase < 0.50 {
            lerp_color(sky_dusk, sky_midnight, (day_phase - 0.25) / 0.25)
        } else if day_phase < 0.75 {
            lerp_color(sky_midnight, sky_dawn, (day_phase - 0.50) / 0.25)
        } else {
            lerp_color(sky_dawn, sky_noon, (day_phase - 0.75) / 0.25)
        };

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
