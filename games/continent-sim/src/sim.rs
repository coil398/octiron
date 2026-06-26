use crate::*;

// ---- simulation tuning ----------------------------------------------------

/// Maximum number of history samples kept in hist_pop / hist_nations.
/// When the buffer is full, the oldest sample is removed from the front.
pub(crate) const HIST_CAP: usize = 128;

pub(crate) const GROWTH_RATE: f32 = 0.06;
pub(crate) const PRESSURE: f32 = 0.72;
pub(crate) const MIGRANT_FRAC: f32 = 0.32;
pub(crate) const SEED_MIN_DIST: f32 = 9.0;
pub(crate) const FAMINE_CHANCE: f32 = 0.012;
pub(crate) const LOG_LINES: usize = 16;
pub(crate) const COUNT_MILESTONES: [usize; 6] = [10, 25, 60, 120, 250, 500];

// ---- technology tuning ----------------------------------------------------

/// Tech flag bits (u8 bitset — no external crate needed).
pub(crate) const TECH_IRRIGATION: u8 = 1;
pub(crate) const TECH_SEAFARING: u8 = 2;
pub(crate) const TECH_METALLURGY: u8 = 4;
pub(crate) const TECH_WRITING: u8 = 8;

/// Number of consecutive at-pressure years near a river before irrigation fires.
pub(crate) const IRRIGATION_THRESHOLD: u32 = 8;
/// Consecutive years a settlement must stay prosperous (pop >= 0.8 * K) before
/// it is promoted to a city.  Independent of the irrigation at_cap_years counter
/// so river-adjacent settlements are not blocked.
pub(crate) const CITY_THRESHOLD: u32 = 12;
/// Consecutive stuck years near coast before seafaring fires.
pub(crate) const SEAFARING_THRESHOLD: u32 = 5;
/// Population threshold for writing to unlock.
pub(crate) const WRITING_POP_THRESHOLD: f32 = 600.0;
/// K raise when irrigation unlocks (fraction of K_MAX).
pub(crate) const IRRIGATION_K_BOOST: f32 = 0.25;
/// K raise when metallurgy unlocks (fraction of K_MAX).
pub(crate) const METALLURGY_K_BOOST: f32 = 0.08;

// ---- §8 event-flash tuning -------------------------------------------------

/// How many years an event-flash ring stays visible (fades over this window).
pub(crate) const FLASH_YEARS: u32 = 3;
/// Maximum number of flash entries kept; oldest are dropped when this is exceeded.
pub(crate) const FLASH_MAX: usize = 16;

// ---- disaster tuning -------------------------------------------------------

pub(crate) const VOLCANO_CHANCE: f32 = 0.004;
pub(crate) const QUAKE_CHANCE: f32 = 0.006;
pub(crate) const FLOOD_CHANCE: f32 = 0.008;
pub(crate) const DROUGHT_CHANCE: f32 = 0.005;
pub(crate) const PLAGUE_CHANCE: f32 = 0.007;
pub(crate) const TSUNAMI_CHANCE: f32 = 0.004;
pub(crate) const DISASTER_TINT_YEARS: u32 = 3; // how long the epicentre tint shows
/// Years a tile shows a disaster-scarred variant (burnt/volcanic) after a strike.
pub(crate) const SCORCH_YEARS: u32 = 14;
/// Base probability per nations-cadence window that an afflicted nation spreads
/// plague along a trade road to one partner (decays with age of affliction).
pub(crate) const PLAGUE_SPREAD_BASE: f32 = 0.18;
/// Years after first affliction before spread probability decays to near-zero.
pub(crate) const PLAGUE_SPREAD_DECAY_YEARS: u32 = 40;

// ---- §5 wildfire tuning ----------------------------------------------------

/// Base chance per year that a Forest tile spontaneously ignites (no drought).
pub(crate) const WILDFIRE_CHANCE_BASE: f32 = 0.006;
/// Elevated chance when a drought struck in the last WILDFIRE_DROUGHT_WINDOW years.
pub(crate) const WILDFIRE_CHANCE_DROUGHT: f32 = 0.025;
/// Years after a drought during which wildfire risk is elevated.
pub(crate) const WILDFIRE_DROUGHT_WINDOW: u32 = 8;
/// How long burned tiles stay scorched (years).
pub(crate) const WILDFIRE_SCORCH_YEARS: u32 = 10;
/// Hard cap on the number of tiles the fire can spread to (including ignition tile).
pub(crate) const WILDFIRE_MAX_SPREAD: usize = 6;

// ---- §6 map editor / god intervention tuning --------------------------------

/// Starting faith pool.
pub(crate) const FAITH_MAX: f32 = 100.0;
/// Faith recovered per in-game year.
pub(crate) const FAITH_REGEN: f32 = 0.8;
/// Faith cost to trigger a disaster at the cursor.
pub(crate) const FAITH_COST_DISASTER: f32 = 30.0;
/// Faith cost to paint/change terrain type.
pub(crate) const FAITH_COST_PAINT: f32 = 5.0;
/// Faith cost to raise/lower elevation (K nudge).
pub(crate) const FAITH_COST_ELEVATE: f32 = 3.0;
/// Faith cost to "grant grace" (gentle growth nudge) to a settlement.
pub(crate) const FAITH_COST_GRACE: f32 = 8.0;
/// Faith cost to "inspire" a settlement with divine blessing (temporary growth boost).
pub(crate) const FAITH_COST_INSPIRE: f32 = 10.0;
/// Faith cost to found a new settlement on an unoccupied habitable tile (§6 Found).
pub(crate) const FAITH_COST_FOUND: f32 = 20.0;
/// K delta when raising/lowering elevation with scroll in edit mode.
pub(crate) const ELEVATION_K_DELTA: f32 = 0.05; // fraction of K_MAX per scroll tick

// ---- nations tuning --------------------------------------------------------

/// Max tile-grid distance between two settlements for them to be "adjacent"
/// enough to be part of the same nation.
pub(crate) const COHESION_DIST: i32 = 2;
/// Max tile-grid distance for a trade link to form between two nations.
/// Intentionally larger than COHESION_DIST so nations that are too far apart
/// to merge can still reach each other for trade — enabling >= 2 trade partners
/// and therefore golden ages.  COHESION_DIST=2 means any settlement within 2
/// tiles collapses into the same nation; trade requires distance > 2 to find a
/// genuinely separate partner, so TRADE_DIST must be >= COHESION_DIST + 1.
/// 6 tiles spans roughly half the inter-nation gap on a typical map while still
/// keeping trade rare enough that >= 2 partners remains special.
pub(crate) const TRADE_DIST: i32 = 6;
/// Base max territory (settlement count) before an admin overload split.
pub(crate) const ADMIN_BASE_LIMIT: usize = 12;
/// Bonus territory slots granted by WRITING tech.
pub(crate) const ADMIN_WRITING_BONUS: usize = 8;
/// Additional admin slots per era index [Stone=0, Neolithic=1, Ancient=2, Medieval=3, Modern=4].
/// Allows an empire to grow large enough to trigger Unification in later eras.
/// Max total: 12 + 8 (writing) + 32 (modern) = 52, well below the 200 sanity cap.
pub(crate) const ADMIN_ERA_BONUS: [usize; 5] = [0, 0, 8, 16, 32];
/// Fraction of all settlements a single nation must hold to trigger Unification.
pub(crate) const UNIFICATION_THRESHOLD: f32 = 0.6;
/// step_nations runs every N years (keep cheap).
pub(crate) const NATIONS_CADENCE: u32 = 4;
/// Minimum connected-component size to found a new nation.
pub(crate) const NATION_FOUND_THRESHOLD: usize = 3;

// ---- PARKS / traits tuning (§4) -------------------------------------------

/// Trait bit: survived a flood → flood-resilience + river-city bonus.
pub(crate) const TRAIT_FLOOD_RESILIENCE: u32 = 1;
/// Trait bit: mountain isolation → masonry + cold-resist.
pub(crate) const TRAIT_MASONRY: u32 = 2;
/// Trait bit: cold-adapted people.
pub(crate) const TRAIT_COLD_RESIST: u32 = 4;
/// Trait bit: coastal trade pressure → seafaring commerce.
pub(crate) const TRAIT_SEAFARING_TRAIT: u32 = 8;
/// Trait bit: sustained trade → commerce bonus.
pub(crate) const TRAIT_COMMERCE: u32 = 16;
/// Trait bit: long war forges discipline.
pub(crate) const TRAIT_DISCIPLINE: u32 = 32;

/// Number of war years that a nation must accumulate before DISCIPLINE is earned.
/// WAR_CADENCE=4 so years ticks 0,4,8,...; threshold 4 fires after 1 round (4 years).
/// Kept equal to WAR_CADENCE so any war that survives at least one full cadence cycle
/// qualifies — this ensures DISCIPLINE can fire naturally across diverse seeds.
pub(crate) const DISCIPLINE_WAR_THRESHOLD: u32 = 4;

// ---- §9 war weariness tuning -----------------------------------------------

/// Years a war must last before the involved nations suffer growth exhaustion.
pub(crate) const WAR_WEARINESS_THRESHOLD: u32 = 10;
/// Growth multiplier applied to ALL settlements of a nation that is in a long
/// war (years >= WAR_WEARINESS_THRESHOLD).  Kept small and bounded so it
/// stacks cleanly with golden-age / inspire multipliers without going to zero.
pub(crate) const WAR_WEARINESS_MULT: f32 = 0.9;

// ---- war tuning ------------------------------------------------------------

/// Base probability per year that two bordering nations start a war.
pub(crate) const WAR_START_CHANCE: f32 = 0.03;
/// Fraction of the attacker's power advantage converted to border-tile advance
/// probability per war tick.
pub(crate) const ADVANCE_RATE: f32 = 0.018;
/// Supply decay per tile of grid distance from the defender's capital (multiplied
/// into defender local strength).
pub(crate) const SUPPLY_DECAY: f32 = 0.035;
/// step_war runs every N years alongside step_nations.
pub(crate) const WAR_CADENCE: u32 = 4;
/// Minimum pop loss on attacker when seizing a tile.
pub(crate) const WAR_POP_LOSS_ATTACKER: f32 = 0.92;
/// Minimum pop loss on defender when losing a tile.
pub(crate) const WAR_POP_LOSS_DEFENDER: f32 = 0.75;

/// Nation name pool (combined with a numeric suffix when exhausted).
pub(crate) const NATION_NAMES: [&str; 16] = [
    "Arath", "Belen", "Corvin", "Duren", "Elvar",
    "Fendar", "Goreth", "Halvur", "Irenn", "Joral",
    "Kaleth", "Loryn", "Morven", "Naral", "Orvyn", "Pelcar",
];

pub(crate) const NEIGHBORS8: [(i32, i32); 8] = [
    (-1, -1), (0, -1), (1, -1),
    (-1, 0), (1, 0),
    (-1, 1), (0, 1), (1, 1),
];

/// Returns the screen-space origin and size for tile (col, row) given the
/// current view state (zoom + pan).
///
/// `screen = (MAP_X, MAP_Y) + ((col, row) - view_pan) * (TILE * view_zoom)`
/// `size   = TILE * view_zoom`
#[inline]
pub(crate) fn view_tile(col: usize, row: usize, pan: Vec2, zoom: f32) -> (f32, f32, f32) {
    let size = TILE * zoom;
    let x = MAP_X + (col as f32 - pan.x) * size;
    let y = MAP_Y + (row as f32 - pan.y) * size;
    (x, y, size)
}

// ---- core data structures --------------------------------------------------

#[derive(Clone, Copy)]
pub(crate) struct Tile {
    pub(crate) terrain: Terrain,
    pub(crate) elevation: f32,
    pub(crate) k: f32,
    pub(crate) habitable: bool,
}

pub(crate) struct Settlement {
    pub(crate) tile: usize,
    pub(crate) pop: f32,
    pub(crate) stuck: u32,
    /// Number of consecutive years this settlement has been at K-pressure
    /// while river-adjacent (irrigation trigger counter).
    pub(crate) at_cap_years: u32,
    /// Consecutive years this settlement has been prosperous (pop >= 0.8 * K).
    /// Used exclusively for city promotion; independent of at_cap_years so that
    /// river-adjacent and irrigated settlements are not blocked by the irrigation
    /// tech counter being reset.
    pub(crate) prosper_years: u32,
    /// Bitset of discovered technologies (TECH_* constants).
    pub(crate) tech: u8,
    /// Earned trait bitset (TRAIT_* constants, §4 PARKS).
    pub(crate) traits: u32,
    /// Count of disasters survived (pop damaged but not killed, §4 PARKS earn counter).
    pub(crate) disasters_survived: u32,
    /// Total years this settlement has been involved in an active war (§4 PARKS).
    pub(crate) war_years: u32,
    /// True once this settlement has grown into a city (prosper_years >= CITY_THRESHOLD
    /// and pop >= 0.8 * tile.k).  Cities receive a small effective-K bonus.
    pub(crate) city: bool,
    /// Years of divine inspiration remaining (§6 Inspire intervention).
    /// While > 0 the settlement receives a ×1.15 growth multiplier; decremented
    /// each tick until it reaches 0.
    pub(crate) inspired_years: u32,
}

// ---- Continent sim methods -------------------------------------------------

impl Continent {
    /// Enqueues a flash highlight at `tile` for the current year.
    /// Index-safe: silently ignores out-of-bounds tiles.
    /// Caps the queue at FLASH_MAX by dropping the oldest entry.
    pub(crate) fn push_flash(&mut self, tile: usize) {
        if tile >= self.tiles.len() {
            return;
        }
        let year = self.year;
        if self.flashes.len() >= FLASH_MAX {
            self.flashes.remove(0);
        }
        self.flashes.push((tile, year));
    }

    /// Returns true if any NEIGHBORS8 neighbour of tile `i` has the given terrain.
    pub(crate) fn tile_adjacent_to(&self, i: usize, target: Terrain) -> bool {
        let col = i % COLS;
        let row = i / COLS;
        for &(dc, dr) in NEIGHBORS8.iter() {
            let nc = col as i32 + dc;
            let nr = row as i32 + dr;
            if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                continue;
            }
            if self.tiles[idx(nc as usize, nr as usize)].terrain == target {
                return true;
            }
        }
        false
    }

    /// Returns true if any NEIGHBORS8 neighbour of tile `i` is a water tile
    /// of type Ocean or DeepOcean (i.e. a coastal tile).
    pub(crate) fn tile_is_coastal(&self, i: usize) -> bool {
        let col = i % COLS;
        let row = i / COLS;
        for &(dc, dr) in NEIGHBORS8.iter() {
            let nc = col as i32 + dc;
            let nr = row as i32 + dr;
            if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                continue;
            }
            let t = self.tiles[idx(nc as usize, nr as usize)].terrain;
            if matches!(t, Terrain::Ocean | Terrain::DeepOcean) {
                return true;
            }
        }
        false
    }

    pub(crate) fn tile_near_river(&self, i: usize) -> bool {
        let col = i % COLS;
        let row = i / COLS;
        for &(dc, dr) in NEIGHBORS8.iter() {
            let nc = col as i32 + dc;
            let nr = row as i32 + dr;
            if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                continue;
            }
            if self.tiles[idx(nc as usize, nr as usize)].terrain == Terrain::River {
                return true;
            }
        }
        false
    }

    /// Finds the best unoccupied habitable neighbour of `tile` (highest K,
    /// above MIN_HABITABLE, not mountain/water).  Shared by expansion and
    /// forced-migration from disasters.
    pub(crate) fn best_free_neighbour(&self, tile: usize) -> Option<usize> {
        let col = tile % COLS;
        let row = tile / COLS;
        let mut best = usize::MAX;
        let mut best_k = MIN_HABITABLE;
        for &(dc, dr) in NEIGHBORS8.iter() {
            let nc = col as i32 + dc;
            let nr = row as i32 + dr;
            if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                continue;
            }
            let ni = idx(nc as usize, nr as usize);
            let t = self.tiles[ni].terrain;
            if t == Terrain::Mountain || t.is_water() {
                continue;
            }
            if !self.tiles[ni].habitable || self.occupied[ni] >= 0 {
                continue;
            }
            if self.tiles[ni].k > best_k {
                best_k = self.tiles[ni].k;
                best = ni;
            }
        }
        if best == usize::MAX { None } else { Some(best) }
    }

    /// Returns NEIGHBORS8 tile indices that exist within the grid.
    pub(crate) fn neighbours(&self, tile: usize) -> Vec<usize> {
        let col = tile % COLS;
        let row = tile / COLS;
        let mut out = Vec::new();
        for &(dc, dr) in NEIGHBORS8.iter() {
            let nc = col as i32 + dc;
            let nr = row as i32 + dr;
            if nc >= 0 && nr >= 0 && nc < COLS as i32 && nr < ROWS as i32 {
                out.push(idx(nc as usize, nr as usize));
            }
        }
        out
    }

    /// Forces the settlement at `tile` to abandon and migrate to the best free
    /// neighbour.  Returns the new tile index (if a destination was found).
    pub(crate) fn force_migrate(&mut self, tile: usize) -> Option<usize> {
        let si = self.occupied[tile];
        if si < 0 {
            return None;
        }
        let dest = self.best_free_neighbour(tile)?;
        let pop = self.settlements[si as usize].pop;
        self.occupied[tile] = -1;
        self.settlements[si as usize].tile = dest;
        self.settlements[si as usize].pop = pop * 0.6; // survival loss
        self.settlements[si as usize].stuck = 0;
        self.occupied[dest] = si;
        // If this settlement is its nation's capital, move the capital TILE with
        // it — otherwise step_nations would spuriously flag a capital loss just
        // because the capital town relocated (disaster/edit-forced migration).
        let nid = self.nation_of.get(si as usize).copied().unwrap_or(-1);
        if nid >= 0 {
            if let Some(n) = self.nations.iter_mut().find(|n| n.id as i32 == nid) {
                if n.capital == tile {
                    n.capital = dest;
                }
            }
        }
        Some(dest)
    }

    /// Growth rate multiplier from trait modifiers.
    /// COMMERCE: small growth bonus.  COLD_RESIST: not modelled here (affects
    /// Tundra expansion in best_free_neighbour).  Others: 1.0.
    pub(crate) fn growth_mult(traits: u32) -> f32 {
        let mut m = 1.0f32;
        if traits & TRAIT_COMMERCE != 0 {
            m += 0.05; // 5% growth bonus
        }
        m
    }

    /// Advances one simulation year: grow toward K, then relieve pressure by
    /// migrating up the K gradient (or stall / famine where the terrain bites).
    pub(crate) fn tick(&mut self) {
        self.year += 1;

        // §5 disasters fire first (before growth so K edits land before
        // logistic growth reads K).
        self.step_disasters();

        let n = self.settlements.len();

        // growth (logistic toward the tile's carrying capacity).
        // Era provides a small growth bonus: each age index adds +2% to GROWTH_RATE.
        let era_mult = 1.0 + self.current_era().index() as f32 * 0.02;
        for i in 0..n {
            let t = self.settlements[i].tile;
            let k = self.tiles[t].k;
            if k > 0.0 {
                let pop = self.settlements[i].pop;
                // §4 PARKS: COMMERCE trait provides a small growth multiplier.
                let gmult = Continent::growth_mult(self.settlements[i].traits);
                // Trade bonus: +2% growth per peaceful trade partner, capped at +10%.
                // Golden-age bonus: +6% growth (bounded, applied if nation is golden).
                let (trade_mult, golden_mult) = if i < self.nation_of.len() {
                    let nid = self.nation_of[i];
                    if nid >= 0 {
                        let nation_opt = self.nations.iter().find(|n| n.id as i32 == nid);
                        let partners = nation_opt.map(|n| n.trade_partners).unwrap_or(0);
                        let is_golden = nation_opt.map(|n| n.golden).unwrap_or(false);
                        let tm = (1.0 + 0.02 * partners as f32).min(1.10);
                        let gm = if is_golden { 1.06f32 } else { 1.0 };
                        (tm, gm)
                    } else {
                        (1.0, 1.0)
                    }
                } else {
                    (1.0, 1.0)
                };
                // City bonus: cities use effective K * 1.1 so the logistic ceiling
                // is slightly higher, giving a small but bounded growth advantage.
                let eff_k = if self.settlements[i].city { k * 1.1 } else { k };
                // §6 Inspire: temporary divine growth boost (×1.15 while inspired_years > 0).
                let inspire_mult = if self.settlements[i].inspired_years > 0 {
                    self.settlements[i].inspired_years -= 1;
                    1.15f32
                } else {
                    1.0
                };
                // §9 War weariness: settlements in a nation engaged in a long war
                // (years >= WAR_WEARINESS_THRESHOLD) suffer a small, bounded growth
                // penalty.  Multiplied in last so it stacks cleanly with all above.
                let weariness_mult = if i < self.nation_of.len() {
                    let nid = self.nation_of[i];
                    if nid >= 0 {
                        let nid_u = nid as u32;
                        let in_long_war = self.wars.iter().any(|w| {
                            (w.a == nid_u || w.b == nid_u) && w.years >= WAR_WEARINESS_THRESHOLD
                        });
                        if in_long_war { WAR_WEARINESS_MULT } else { 1.0 }
                    } else {
                        1.0
                    }
                } else {
                    1.0
                };
                let next = pop + GROWTH_RATE * era_mult * gmult * trade_mult * golden_mult * inspire_mult * weariness_mult * pop * (1.0 - pop / eff_k);
                self.settlements[i].pop = next.max(1.0);
            }
        }

        // City promotion: a settlement becomes a city when prosper_years >=
        // CITY_THRESHOLD and pop >= 0.8 * tile.k.  Once promoted it stays a city.
        // prosper_years is a dedicated counter separate from at_cap_years (which
        // step_tech owns for irrigation), so river-adjacent and irrigated
        // settlements are never blocked from promotion.
        {
            let year = self.year;
            let mut city_log: Vec<String> = Vec::new();
            for i in 0..self.settlements.len() {
                if self.settlements[i].city {
                    continue; // already a city — nothing to update
                }
                let t = self.settlements[i].tile;
                let k = self.tiles[t].k;
                if k <= 0.0 {
                    continue;
                }
                let pop = self.settlements[i].pop;
                let prosperous = pop >= 0.8 * k;
                // Increment or decay prosper_years for every settlement.
                if prosperous {
                    self.settlements[i].prosper_years += 1;
                } else {
                    self.settlements[i].prosper_years = 0;
                }
                // Promotion check (any settlement regardless of river adjacency).
                if self.settlements[i].prosper_years >= CITY_THRESHOLD && prosperous {
                    self.settlements[i].city = true;
                    city_log.push(format!("Y{} a great city rises", year));
                }
            }
            for msg in city_log {
                self.log_event(msg);
            }
        }

        // expansion (collect new towns, append after the pass).
        let mut spawned: Vec<Settlement> = Vec::new();
        for i in 0..n {
            let tile = self.settlements[i].tile;
            let pop = self.settlements[i].pop;
            let k = self.tiles[tile].k;

            // resourcefx (a): Horses tile lowers the effective pressure threshold
            // so the settlement expands sooner, and raises the migrant fraction slightly.
            let on_horses = tile < self.resources.len() && self.resources[tile] == Resource::Horses;
            let effective_pressure = if on_horses { PRESSURE - 0.04 } else { PRESSURE };
            let effective_migrant_frac = if on_horses { MIGRANT_FRAC + 0.03 } else { MIGRANT_FRAC };

            if k <= 0.0 || pop / k < effective_pressure {
                continue;
            }

            // Detect blocks for the stall log (mountains/seas)
            let col = tile % COLS;
            let row = tile / COLS;
            let mut saw_block = false;
            for &(dc, dr) in NEIGHBORS8.iter() {
                let nc = col as i32 + dc;
                let nr = row as i32 + dr;
                if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                    continue;
                }
                let ni = idx(nc as usize, nr as usize);
                let t = self.tiles[ni].terrain;
                if t == Terrain::Mountain || t.is_water() {
                    saw_block = true;
                }
            }

            // Use the shared best_free_neighbour method
            let best = self.best_free_neighbour(tile);

            if let Some(best) = best {
                let migrants = pop * effective_migrant_frac;
                self.settlements[i].pop = pop - migrants;
                self.settlements[i].stuck = 0;
                let future = (n + spawned.len()) as i32;
                self.occupied[best] = future;
                // §8 event-flash: new settlement founding.
                self.push_flash(best);
                spawned.push(Settlement {
                    tile: best,
                    pop: migrants,
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
                if !self.river_city_logged && self.tile_near_river(best) {
                    self.river_city_logged = true;
                    let year = self.year;
                    self.log_event(format!("Y{} a people settles the great river", year));
                }
            } else {
                self.settlements[i].stuck += 1;
                if saw_block && !self.stall_logged && self.settlements[i].stuck > 3 {
                    self.stall_logged = true;
                    let year = self.year;
                    self.log_event(format!("Y{} mountains and seas halt the spread", year));
                }
                if self.roll() < FAMINE_CHANCE {
                    self.settlements[i].pop *= 0.7;
                    if self.roll() < 0.25 {
                        let year = self.year;
                        self.log_event(format!("Y{} famine thins a crowded town", year));
                    }
                }
            }
        }

        self.settlements.extend(spawned);

        // §3 technology fires AFTER expansion so stuck/at-cap state is current.
        self.step_tech();

        // §2 nations: runs every NATIONS_CADENCE years (coarse, cheap BFS).
        if self.year % NATIONS_CADENCE == 0 {
            self.step_nations();
            // Trade partner counts depend on current war state; run after step_nations
            // so territory lists and war state are both fresh.
            self.step_trade();
        }

        // §9 war: runs every WAR_CADENCE years after nations are updated.
        if self.year % WAR_CADENCE == 0 {
            self.step_war();
        }

        // §4 PARKS / traits: propagate traits and award discipline each tick.
        self.step_parks();

        // §6 god intervention: regenerate faith each year.
        self.regen_faith();

        // §8 event-flash: purge expired entries (age >= FLASH_YEARS).
        let year_now = self.year;
        self.flashes.retain(|&(_, started)| year_now.saturating_sub(started) < FLASH_YEARS);

        // ---- climate drift --------------------------------------------------
        // Drift oscillates slowly using a sine of the year (period ~120 years),
        // bounded in [-0.15, 0.15].  The sine gives a smooth, reversing motion.
        self.climate_drift = 0.15 * (self.year as f32 / 19.0).sin();

        // Every ~25 years recompute land-tile habitability with the updated drift.
        // IMPORTANT: we do NOT overwrite tile.k from terrain — that would erase
        // tech/edit/disaster boosts accumulated on tile.k.  Instead we use the
        // stored base_k (pure terrain capacity at world-gen) and scale by the
        // ratio of effective-climate to base-climate, then only update habitability
        // and tile.k on tiles that GAIN or LOSE habitability.
        if self.year % 25 == 0 {
            let count = COLS * ROWS;
            let drift = self.climate_drift;
            let mut newly_uninhabitable: Vec<usize> = Vec::new();

            for i in 0..count {
                let t = self.tiles[i].terrain;
                if t.is_water() || t == Terrain::Mountain {
                    continue;
                }
                let base_climate = if i < self.climate.len() { self.climate[i] } else { 0.65 };
                // Avoid divide-by-zero; base is guaranteed >= 0.18 from generate().
                let base = base_climate.max(0.01);
                let eff = (base_climate + drift).max(0.01);
                // Climate-scaled K from the pure terrain baseline (no boosts removed).
                let base_k_tile = if i < self.base_k.len() { self.base_k[i] } else { 0.0 };
                let climate_k = (base_k_tile * (eff / base)).clamp(0.0, K_MAX * 1.4);
                let was_hab = self.tiles[i].habitable;
                let now_hab = climate_k >= MIN_HABITABLE;

                if was_hab && !now_hab {
                    // Tile LOSES habitability: zero out K and force-migrate.
                    self.tiles[i].k = 0.0;
                    self.tiles[i].habitable = false;
                    self.habitable_count = self.habitable_count.saturating_sub(1);
                    newly_uninhabitable.push(i);
                } else if !was_hab && now_hab {
                    // Tile GAINS habitability: set K to the climate-scaled value.
                    self.tiles[i].k = climate_k;
                    self.tiles[i].habitable = true;
                    self.habitable_count += 1;
                }
                // else: habitability unchanged — leave tile.k as-is (preserves
                // all tech/edit/disaster boosts on this tile).
            }

            // Force-migrate any settlements that lost their habitability.
            for tile in newly_uninhabitable {
                if self.occupied[tile] >= 0 {
                    self.force_migrate(tile);
                }
            }

            // Log the climate shift occasionally (every 50 years at the 25-year mark).
            if self.year % 50 == 25 || self.year % 100 == 0 {
                let year = self.year;
                let msg = if drift > 0.05 {
                    format!("Y{} the climate warms, the frontier shifts", year)
                } else if drift < -0.05 {
                    format!("Y{} the climate cools, the frontier shifts", year)
                } else {
                    format!("Y{} a mild season, the frontier holds", year)
                };
                self.log_event(msg);
            }
        }

        // ---- milestone / era / unification / extinction events ----------------
        self.step_milestones();

        // ---- §8 history buffers (observation-only, NEVER read by sim logic) ---
        // Record total population and nation count once per year.
        // These buffers are bounded at HIST_CAP; the oldest sample is evicted when
        // full.  They are never read by tick/step_*/sim_rng — observation only.
        {
            let total_pop: f32 = self.settlements.iter().map(|s| s.pop).sum();
            let nation_count = self.nations.len() as u16;
            if self.hist_pop.len() >= HIST_CAP {
                self.hist_pop.remove(0);
            }
            self.hist_pop.push(total_pop);
            if self.hist_nations.len() >= HIST_CAP {
                self.hist_nations.remove(0);
            }
            self.hist_nations.push(nation_count);
        }
    }
}
