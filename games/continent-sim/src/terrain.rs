use crate::*;

// ---- world generation tuning ---------------------------------------------

pub(crate) const MOUNTAIN_LEVEL: f32 = 0.72;
pub(crate) const K_MAX: f32 = 1000.0;
pub(crate) const MIN_HABITABLE: f32 = 55.0;

// ---- tile index helper ---------------------------------------------------

#[inline]
pub(crate) fn idx(col: usize, row: usize) -> usize {
    row * COLS + col
}

// ---- value-noise terrain --------------------------------------------------

#[inline]
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

pub(crate) fn hash01(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = seed ^ 0x9e37_79b9;
    h = h.wrapping_add((x as u32).wrapping_mul(0x27d4_eb2d));
    h ^= h >> 15;
    h = h.wrapping_mul(0x85eb_ca6b);
    h = h.wrapping_add((y as u32).wrapping_mul(0x1656_67b1));
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

fn value_noise(fx: f32, fy: f32, seed: u32) -> f32 {
    let x0 = fx.floor();
    let y0 = fy.floor();
    let xi = x0 as i32;
    let yi = y0 as i32;
    let tx = smoothstep(fx - x0);
    let ty = smoothstep(fy - y0);
    let v00 = hash01(xi, yi, seed);
    let v10 = hash01(xi + 1, yi, seed);
    let v01 = hash01(xi, yi + 1, seed);
    let v11 = hash01(xi + 1, yi + 1, seed);
    let a = v00 + (v10 - v00) * tx;
    let b = v01 + (v11 - v01) * tx;
    a + (b - a) * ty
}

/// Fractal (multi-octave) value noise in `0.0..=1.0`.
fn fbm(nx: f32, ny: f32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for o in 0..5u32 {
        sum += value_noise(nx * freq, ny * freq, seed.wrapping_add(o.wrapping_mul(7919))) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

// ---- resources ------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Resource {
    None,
    Iron,
    Horses,
    Gold,
}

impl Resource {
    pub(crate) fn label_jp(self) -> &'static str {
        match self {
            Resource::None   => "",
            Resource::Iron   => "鉄",
            Resource::Horses => "馬",
            Resource::Gold   => "金",
        }
    }
}

// ---- terrain --------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Terrain {
    DeepOcean,
    Ocean,
    River,
    Plains,
    Forest,
    Hills,
    Mountain,
    Desert,
    Tundra,
}

impl Terrain {
    pub(crate) fn fertility(self) -> f32 {
        match self {
            Terrain::Plains => 1.0,
            Terrain::Forest => 0.65,
            Terrain::Hills => 0.45,
            Terrain::Tundra => 0.18,
            Terrain::Desert => 0.08,
            Terrain::Mountain => 0.12,
            _ => 0.0,
        }
    }

    pub(crate) fn is_water(self) -> bool {
        matches!(self, Terrain::DeepOcean | Terrain::Ocean | Terrain::River)
    }

    pub(crate) fn base_color(self) -> Color {
        match self {
            Terrain::DeepOcean => [0.05, 0.10, 0.26, 1.0],
            Terrain::Ocean => [0.10, 0.22, 0.45, 1.0],
            Terrain::River => [0.24, 0.50, 0.88, 1.0],
            Terrain::Plains => [0.45, 0.63, 0.32, 1.0],
            Terrain::Forest => [0.20, 0.42, 0.22, 1.0],
            Terrain::Hills => [0.47, 0.50, 0.30, 1.0],
            Terrain::Mountain => [0.45, 0.43, 0.46, 1.0],
            Terrain::Desert => [0.80, 0.72, 0.44, 1.0],
            Terrain::Tundra => [0.72, 0.76, 0.80, 1.0],
        }
    }

    /// Atlas sprite name for this terrain (matches the keys in atlas.txt).
    pub(crate) fn tile_name(self) -> &'static str {
        match self {
            Terrain::DeepOcean => "deepocean",
            Terrain::Ocean => "ocean",
            Terrain::River => "river",
            Terrain::Plains => "plains",
            Terrain::Forest => "forest",
            Terrain::Hills => "hills",
            Terrain::Mountain => "mountain",
            Terrain::Desert => "desert",
            Terrain::Tundra => "tundra",
        }
    }

    pub(crate) fn tile_name_jp(self) -> &'static str {
        match self {
            Terrain::DeepOcean => "深海",
            Terrain::Ocean     => "海",
            Terrain::River     => "川",
            Terrain::Plains    => "草原",
            Terrain::Forest    => "森林",
            Terrain::Hills     => "丘陵",
            Terrain::Mountain  => "山岳",
            Terrain::Desert    => "砂漠",
            Terrain::Tundra    => "ツンドラ",
        }
    }

    pub(crate) fn index(self) -> usize {
        match self {
            Terrain::DeepOcean => 0,
            Terrain::Ocean => 1,
            Terrain::River => 2,
            Terrain::Plains => 3,
            Terrain::Forest => 4,
            Terrain::Hills => 5,
            Terrain::Mountain => 6,
            Terrain::Desert => 7,
            Terrain::Tundra => 8,
        }
    }
}

pub(crate) const TERRAIN_COUNT: usize = 9;
pub(crate) const ALL_TERRAINS: [Terrain; TERRAIN_COUNT] = [
    Terrain::DeepOcean,
    Terrain::Ocean,
    Terrain::River,
    Terrain::Plains,
    Terrain::Forest,
    Terrain::Hills,
    Terrain::Mountain,
    Terrain::Desert,
    Terrain::Tundra,
];

// ---- §7 world-generation presets -------------------------------------------

/// Tunable knobs the scenario presets vary in `generate()`.
#[derive(Clone, Copy)]
pub(crate) struct ScenarioParams {
    pub(crate) sea_level: f32,
    pub(crate) hill_level: f32,
    pub(crate) mountain_level: f32,
    pub(crate) desert_moist: f32,
    pub(crate) forest_moist: f32,
    pub(crate) feature: f32,
    pub(crate) seed_count: usize,
}

/// Map-generation preset, cycled with [P]. Each yields a recognisably different
/// world so "draw a different map, get a different history" holds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scenario {
    Standard,
    Pangaea,
    Archipelago,
    Highlands,
    Arid,
    /// Cold climate: high sea level, moisture biased toward tundra, lower K ceiling
    /// achieved via narrow forest/plains bands and a pushed-up desert_moist threshold.
    IceAge,
    /// Tectonically active: extra mountains, moderate landmass, elevated disaster rate
    /// reflected by steeper hill/mountain thresholds and compact feature scale.
    Volcanic,
    /// Tropical paradise: low sea level, wide forest/plains band, river-rich lowlands,
    /// gentle latitude modulation so equatorial tiles stay warm and fertile.
    Lush,
}

impl Scenario {
    pub(crate) fn next(self) -> Scenario {
        match self {
            Scenario::Standard    => Scenario::Pangaea,
            Scenario::Pangaea     => Scenario::Archipelago,
            Scenario::Archipelago => Scenario::Highlands,
            Scenario::Highlands   => Scenario::Arid,
            Scenario::Arid        => Scenario::IceAge,
            Scenario::IceAge      => Scenario::Volcanic,
            Scenario::Volcanic    => Scenario::Lush,
            Scenario::Lush        => Scenario::Standard,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Scenario::Standard    => "STANDARD",
            Scenario::Pangaea     => "PANGAEA",
            Scenario::Archipelago => "ARCHIPELAGO",
            Scenario::Highlands   => "HIGHLANDS",
            Scenario::Arid        => "ARID",
            Scenario::IceAge      => "ICEAGE",
            Scenario::Volcanic    => "VOLCANIC",
            Scenario::Lush        => "LUSH",
        }
    }

    pub(crate) fn label_jp(self) -> &'static str {
        match self {
            Scenario::Standard    => "標準",
            Scenario::Pangaea     => "パンゲア",
            Scenario::Archipelago => "群島",
            Scenario::Highlands   => "高地",
            Scenario::Arid        => "乾燥",
            Scenario::IceAge      => "氷河期",
            Scenario::Volcanic    => "火山帯",
            Scenario::Lush        => "豊穣",
        }
    }

    pub(crate) fn params(self) -> ScenarioParams {
        match self {
            Scenario::Standard => ScenarioParams {
                sea_level: 0.30, hill_level: 0.56, mountain_level: 0.72,
                desert_moist: 0.32, forest_moist: 0.64, feature: 14.0, seed_count: 5,
            },
            // one broad landmass: lower seas, larger features, more tribes
            Scenario::Pangaea => ScenarioParams {
                sea_level: 0.22, hill_level: 0.58, mountain_level: 0.74,
                desert_moist: 0.34, forest_moist: 0.64, feature: 20.0, seed_count: 6,
            },
            // scattered islands: higher seas, small features, fewer tribes
            Scenario::Archipelago => ScenarioParams {
                sea_level: 0.42, hill_level: 0.55, mountain_level: 0.72,
                desert_moist: 0.30, forest_moist: 0.62, feature: 9.0, seed_count: 4,
            },
            // mountainous: lower hill/mountain thresholds -> more highland
            Scenario::Highlands => ScenarioParams {
                sea_level: 0.30, hill_level: 0.48, mountain_level: 0.62,
                desert_moist: 0.32, forest_moist: 0.66, feature: 13.0, seed_count: 5,
            },
            // arid: wide desert belts, little forest
            Scenario::Arid => ScenarioParams {
                sea_level: 0.30, hill_level: 0.56, mountain_level: 0.72,
                desert_moist: 0.50, forest_moist: 0.78, feature: 14.0, seed_count: 5,
            },
            // ice age: higher seas reduce landmass, cold pushes tundra far south,
            // desert_moist low (little evaporation), forest_moist high (scarce forest),
            // moderate features so habitable plains corridors still exist near the equator.
            Scenario::IceAge => ScenarioParams {
                sea_level: 0.36, hill_level: 0.54, mountain_level: 0.70,
                desert_moist: 0.20, forest_moist: 0.72, feature: 13.0, seed_count: 4,
            },
            // volcanic: lower mountain threshold -> many peaks, moderate landmass,
            // slightly larger features so volcanic ridges form coherent chains.
            Scenario::Volcanic => ScenarioParams {
                sea_level: 0.28, hill_level: 0.44, mountain_level: 0.58,
                desert_moist: 0.34, forest_moist: 0.66, feature: 12.0, seed_count: 5,
            },
            // lush: low sea level exposes broad tropical lowlands, narrow desert band,
            // wide forest corridor, small features encourage river-laced plains.
            Scenario::Lush => ScenarioParams {
                sea_level: 0.24, hill_level: 0.58, mountain_level: 0.74,
                desert_moist: 0.22, forest_moist: 0.55, feature: 11.0, seed_count: 6,
            },
        }
    }
}

// ---- impl Continent: terrain-related methods -------------------------------

impl Continent {
    /// Recomputes K and habitability for a tile using its current terrain and
    /// the neighbours' moisture (approximated from existing K data).  Called
    /// after any terrain paint or elevation edit so the causal chain stays live.
    pub(crate) fn recompute_tile_k(&mut self, i: usize) {
        let t = self.tiles[i].terrain;
        // Water and mountain tiles have no carrying capacity.
        if t.is_water() || t == Terrain::Mountain {
            let was_hab = self.tiles[i].habitable;
            self.tiles[i].k = 0.0;
            self.tiles[i].habitable = false;
            if was_hab {
                self.habitable_count = self.habitable_count.saturating_sub(1);
            }
            return;
        }
        // Approximate freshwater availability by checking for river neighbours.
        let near_river = self.tile_near_river(i);
        let fw = if near_river { 1.0 } else { 0.55 };
        // Use the per-tile base climate plus the current global drift.
        let base_climate = if i < self.climate.len() { self.climate[i] } else { 0.65 };
        let climate = (base_climate + self.climate_drift).max(0.0);
        let new_k = (t.fertility() * fw * climate * K_MAX).clamp(0.0, K_MAX * 1.4);
        let was_hab = self.tiles[i].habitable;
        let now_hab = new_k >= MIN_HABITABLE;
        self.tiles[i].k = new_k;
        self.tiles[i].habitable = now_hab;
        match (was_hab, now_hab) {
            (false, true) => self.habitable_count += 1,
            (true, false) => self.habitable_count = self.habitable_count.saturating_sub(1),
            _ => {}
        }
    }

    /// Builds the whole world from `self.seed` and seeds the founding tribes.
    pub(crate) fn generate(&mut self) {
        let seed = self.seed;
        let count = COLS * ROWS;
        let p = self.scenario.params();

        // 1. elevation / moisture / climate fields.
        let cx = (COLS as f32 - 1.0) * 0.5;
        let cy = (ROWS as f32 - 1.0) * 0.5;
        let feature = p.feature;
        let mut elevation = vec![0.0f32; count];
        let mut moisture = vec![0.0f32; count];
        let mut climate = vec![0.0f32; count];
        for row in 0..ROWS {
            for col in 0..COLS {
                let nx = col as f32 / feature;
                let ny = row as f32 / feature;
                let e = fbm(nx, ny, seed);
                // radial falloff so the map's rim sinks into ocean.
                let dx = (col as f32 - cx) / cx;
                let dy = (row as f32 - cy) / cy;
                let r = (dx * dx + dy * dy).sqrt();
                let mask = (1.18 - 1.10 * r).clamp(0.0, 1.0);
                let m = fbm(nx + 31.7, ny - 12.3, seed.wrapping_add(99_991));
                let lat = row as f32 / (ROWS as f32 - 1.0);
                let band = (1.0 - (2.0 * (lat - 0.5)).abs()).clamp(0.0, 1.0);
                let i = idx(col, row);
                elevation[i] = (e * mask).clamp(0.0, 1.0);
                moisture[i] = m;
                climate[i] = 0.18 + 0.82 * band;
            }
        }

        // 2. classify terrain (before rivers carve through).
        let mut terrain = vec![Terrain::DeepOcean; count];
        for i in 0..count {
            let e = elevation[i];
            terrain[i] = if e < p.sea_level {
                if e < p.sea_level * 0.55 {
                    Terrain::DeepOcean
                } else {
                    Terrain::Ocean
                }
            } else if e > p.mountain_level {
                Terrain::Mountain
            } else if e > p.hill_level {
                Terrain::Hills
            } else if climate[i] < 0.34 {
                Terrain::Tundra
            } else if moisture[i] < p.desert_moist {
                Terrain::Desert
            } else if moisture[i] > p.forest_moist {
                Terrain::Forest
            } else {
                Terrain::Plains
            };
        }

        // 3. rivers via downhill flow accumulation.
        let mut land: Vec<usize> = (0..count).filter(|&i| elevation[i] >= p.sea_level).collect();
        land.sort_by(|&a, &b| {
            elevation[b]
                .partial_cmp(&elevation[a])
                .unwrap_or(Ordering::Equal)
        });
        let mut flow = vec![0.0f32; count];
        for &i in &land {
            flow[i] = 0.6 + 0.8 * moisture[i];
        }
        let mut downhill = vec![usize::MAX; count];
        for &i in &land {
            let col = i % COLS;
            let row = i / COLS;
            let mut best = usize::MAX;
            let mut best_e = elevation[i];
            for &(dc, dr) in NEIGHBORS8.iter() {
                let nc = col as i32 + dc;
                let nr = row as i32 + dr;
                if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                    continue;
                }
                let ni = idx(nc as usize, nr as usize);
                if elevation[ni] < best_e {
                    best_e = elevation[ni];
                    best = ni;
                }
            }
            if best != usize::MAX {
                downhill[i] = best;
                flow[best] += flow[i];
            }
        }
        let river_thresh = (land.len() as f32 / 55.0).clamp(6.0, 28.0);
        for &i in &land {
            if terrain[i] != Terrain::Mountain
                && downhill[i] != usize::MAX
                && flow[i] > river_thresh
            {
                terrain[i] = Terrain::River;
            }
        }

        // 4. freshwater coefficient -> carrying capacity -> habitability.
        let mut tiles = Vec::with_capacity(count);
        let mut habitable_count = 0;
        for row in 0..ROWS {
            for col in 0..COLS {
                let i = idx(col, row);
                let t = terrain[i];
                let mut near_river = matches!(t, Terrain::River);
                if !near_river {
                    for &(dc, dr) in NEIGHBORS8.iter() {
                        let nc = col as i32 + dc;
                        let nr = row as i32 + dr;
                        if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                            continue;
                        }
                        if terrain[idx(nc as usize, nr as usize)] == Terrain::River {
                            near_river = true;
                            break;
                        }
                    }
                }
                let fw = if near_river {
                    1.0
                } else {
                    (0.25 + 0.55 * moisture[i]).min(0.85)
                };
                let k = t.fertility() * fw * climate[i] * K_MAX;
                let hab = !t.is_water() && t != Terrain::Mountain && k >= MIN_HABITABLE;
                if hab {
                    habitable_count += 1;
                }
                tiles.push(Tile {
                    terrain: t,
                    elevation: elevation[i],
                    k,
                    habitable: hab,
                });
            }
        }

        // Build base_k: pure terrain capacity with NO tech/edit/disaster boosts.
        let mut base_k_vec = vec![0.0f32; count];
        for row in 0..ROWS {
            for col in 0..COLS {
                let i = idx(col, row);
                base_k_vec[i] = tiles[i].k;
            }
        }

        self.tiles = tiles;
        self.base_k = base_k_vec;
        self.climate = climate;
        self.climate_drift = 0.0;
        self.occupied = vec![-1i32; count];
        self.settlements = Vec::new();
        self.habitable_count = habitable_count;
        self.year = 0;
        self.prev_year = 0;
        self.accum = 0.0;
        self.log = Vec::new();
        self.next_pop_milestone = 0;
        self.river_city_logged = false;
        self.stall_logged = false;
        self.prev_era = Era::Stone;
        self.era_changed = false;
        self.prev_war_count = 0;
        self.prev_golden_count = 0;
        self.alliance_count = 0;
        self.prev_alliance_count = 0;
        self.prev_city_count = 0;
        self.unification_logged = false;
        self.extinction_logged = false;
        self.recovery = Vec::new();
        self.scorch = vec![0u32; count];
        self.disaster_count = 0;
        self.last_disaster = None;
        self.tech_unlocked = 0;
        self.nations = Vec::new();
        self.nation_of = Vec::new();
        self.next_nation_id = 0;
        self.nation_name_idx = 0;
        self.wars = Vec::new();
        self.lineage_traits = std::collections::HashMap::new();
        self.plague_nations = std::collections::HashMap::new();
        // §6: reset edit mode on new world
        self.edit_mode = EditMode::Observe;
        self.edit_tool = EditTool::PaintTerrain;
        self.faith = FAITH_MAX;
        self.faith_accum = 0.0;
        self.hovered_tile = None;
        self.selected_tile = None;
        // §10: reset zoom/pan on new world
        self.view_zoom = 1.0;
        self.view_pan = Vec2::new(0.0, 0.0);
        // 3D camera: reset orbit angles on new world
        self.view_azimuth = 0.6;
        self.view_elevation = 35.0_f32.to_radians();
        // §8: reset event-flash queue on new world
        self.flashes = Vec::new();
        // §8 history buffers: reset on new world so the graph starts fresh.
        self.hist_pop = Vec::new();
        self.hist_nations = Vec::new();

        // 5. seed founding tribes on the richest, well-spaced ground.
        let mut cand: Vec<usize> = (0..count).filter(|&i| self.tiles[i].habitable).collect();
        cand.sort_by(|&a, &b| {
            self.tiles[b]
                .k
                .partial_cmp(&self.tiles[a].k)
                .unwrap_or(Ordering::Equal)
        });
        let mut chosen: Vec<usize> = Vec::new();
        for &i in &cand {
            let (c1, r1) = (i % COLS, i / COLS);
            let ok = chosen.iter().all(|&j| {
                let (c2, r2) = (j % COLS, j / COLS);
                let dc = c1 as f32 - c2 as f32;
                let dr = r1 as f32 - r2 as f32;
                (dc * dc + dr * dr).sqrt() >= SEED_MIN_DIST
            });
            if ok {
                chosen.push(i);
                if chosen.len() >= p.seed_count {
                    break;
                }
            }
        }
        for &i in &chosen {
            let si = self.settlements.len() as i32;
            self.occupied[i] = si;
            self.settlements.push(Settlement {
                tile: i,
                pop: 25.0,
                stuck: 0,
                at_cap_years: 0,
                prosper_years: 0,
                tech: 0,
                traits: 0,
                disasters_survived: 0,
                war_years: 0,
                city: false,
                inspired_years: 0,
            });
        }
        // Initialise nation_of with -1 for every settlement (no nation yet).
        self.nation_of = vec![-1i32; self.settlements.len()];

        // 6. Place sparse resource deposits deterministically using hash of (i, seed).
        self.resources = vec![Resource::None; count];
        for i in 0..count {
            let t = self.tiles[i].terrain;
            let h1 = hash01(i as i32, seed as i32, seed.wrapping_add(0xdead_beef));
            let h2 = hash01(i as i32, seed as i32, seed.wrapping_add(0xcafe_1234));
            match t {
                // Iron: ~3% of Mountain or Hills tiles.
                Terrain::Mountain | Terrain::Hills if h1 < 0.03 => {
                    self.resources[i] = Resource::Iron;
                }
                // Horses: ~3% of Plains tiles.
                Terrain::Plains if h1 < 0.03 => {
                    self.resources[i] = Resource::Horses;
                }
                // Gold: ~2% of Hills or Desert tiles.
                Terrain::Hills | Terrain::Desert if h2 < 0.02 => {
                    if h1 >= 0.03 {
                        self.resources[i] = Resource::Gold;
                    }
                }
                _ => {}
            }
        }

        let founded = self.settlements.len();
        self.log_event(format!("Y0 {} tribes take root", founded));
    }
}
