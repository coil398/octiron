use crate::*;

// ---- KEdit ----------------------------------------------------------------

/// A pending terrain/K rewrite enqueued by a disaster (volcanic ash, flood
/// alluvium).  Applied after `years_left` reaches zero so the player sees the
/// cause (disaster log entry) THEN the effect (soil-enrichment log entry).
#[derive(Clone)]
pub(crate) struct KEdit {
    pub(crate) tile: usize,
    pub(crate) delta: f32,
    pub(crate) years_left: u32,
}

// ---- DisasterKind ---------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DisasterKind {
    Volcano,
    Quake,
    Flood,
    Drought,
    Plague,
    Tsunami,
}

impl DisasterKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            DisasterKind::Volcano => "volcano",
            DisasterKind::Quake => "earthquake",
            DisasterKind::Flood => "flood",
            DisasterKind::Drought => "drought",
            DisasterKind::Plague => "plague",
            DisasterKind::Tsunami => "tsunami",
        }
    }

    pub(crate) fn label_jp(self) -> &'static str {
        match self {
            DisasterKind::Volcano => "火山噴火",
            DisasterKind::Quake => "地震",
            DisasterKind::Flood => "洪水",
            DisasterKind::Drought => "干ばつ",
            DisasterKind::Plague => "疫病",
            DisasterKind::Tsunami => "津波",
        }
    }
}

// ---- impl Continent (disaster system) -------------------------------------

impl Continent {
    /// Enqueues a pending K raise (ash / alluvium) for all non-mountain,
    /// non-water neighbours of `epicentre`.
    pub(crate) fn enqueue_soil_enrichment(
        &mut self,
        epicentre: usize,
        delta_frac: f32,
        years_left: u32,
    ) {
        let neighbours = self.neighbours(epicentre);
        for ni in neighbours {
            let t = self.tiles[ni].terrain;
            if t == Terrain::Mountain || t.is_water() {
                continue;
            }
            self.recovery.push(KEdit {
                tile: ni,
                delta: delta_frac * K_MAX,
                years_left,
            });
        }
    }

    /// Processes the pending K-edit recovery queue (delayed soil enrichment).
    pub(crate) fn process_recovery(&mut self) {
        let year = self.year;
        let mut applied_positive = Vec::new();

        // Split into ready (years_left == 0) and pending (years_left > 0).
        // We drain ready entries, apply them, and keep only the pending ones.
        let mut next_recovery = Vec::new();
        let ready: Vec<KEdit> = self
            .recovery
            .drain(..)
            .filter_map(|mut e| {
                if e.years_left == 0 {
                    Some(e)
                } else {
                    e.years_left -= 1;
                    next_recovery.push(e);
                    None
                }
            })
            .collect();
        self.recovery = next_recovery;

        for edit in ready {
            let old_k = self.tiles[edit.tile].k;
            let new_k = (old_k + edit.delta).clamp(0.0, K_MAX * 1.4);
            self.tiles[edit.tile].k = new_k;
            // update habitability
            let t = self.tiles[edit.tile].terrain;
            let was_hab = self.tiles[edit.tile].habitable;
            let now_hab = !t.is_water() && t != Terrain::Mountain && new_k >= MIN_HABITABLE;
            self.tiles[edit.tile].habitable = now_hab;
            if now_hab && !was_hab {
                self.habitable_count += 1;
            } else if !now_hab && was_hab {
                self.habitable_count = self.habitable_count.saturating_sub(1);
            }
            if edit.delta > 0.0 {
                applied_positive.push(edit.tile);
            }
        }

        // log the first positive application this year
        if !applied_positive.is_empty() {
            let rep = applied_positive[0];
            let t = self.tiles[rep].terrain;
            let msg = if t == Terrain::Plains || t == Terrain::Forest {
                format!("Y{} silt makes the floodplain bloom", year)
            } else {
                format!("Y{} ash-rich soil revives the slopes", year)
            };
            self.log_event(msg);
        }
    }

    /// Applies a disaster of the given kind centred on `epicentre`.
    /// This is also called by the map-editor intervention (§6) so both
    /// periodic and manual paths share the same effect code.
    pub(crate) fn apply_disaster(&mut self, kind: DisasterKind, epicentre: usize) {
        self.disaster_count += 1;
        let year = self.year;
        self.last_disaster = Some((epicentre, kind, year));
        // §8 event-flash: mark the disaster epicentre.
        self.push_flash(epicentre);

        // Scar the land around the epicentre so tiles visibly change (§5).
        let scar_until = year + SCORCH_YEARS;
        self.scorch[epicentre] = scar_until;
        for ni in self.neighbours(epicentre) {
            self.scorch[ni] = scar_until;
        }

        match kind {
            DisasterKind::Volcano => {
                // Effect 1: destroy/damage settlements within radius 2.
                // Collect tiles up-front to avoid holding borrow during mutation.
                let volcano_tiles: Vec<usize> = std::iter::once(epicentre)
                    .chain(self.neighbours(epicentre).iter().copied())
                    .collect();
                // Collect (si, survive_frac) first to avoid multiple rolls during
                // the borrow of self.occupied / self.settlements.
                let mut volcano_hits: Vec<(usize, f32)> = Vec::new();
                for &ni in &volcano_tiles {
                    let si = self.occupied[ni];
                    if si >= 0 {
                        let survive_frac = self.roll() * 0.4;
                        volcano_hits.push((si as usize, survive_frac));
                    }
                }
                for (si, survive_frac) in volcano_hits {
                    let pop = self.settlements[si].pop;
                    if survive_frac < 0.15 {
                        // direct hit — force migrate survivors
                        let tile = self.settlements[si].tile;
                        self.force_migrate(tile);
                    } else {
                        self.settlements[si].pop = pop * survive_frac;
                        // §4 PARKS: survived a mountain disaster → earn MASONRY
                        self.settlements[si].disasters_survived += 1;
                        let tile = self.settlements[si].tile;
                        // Mountain-adjacent or isolated → MASONRY + COLD_RESIST
                        let near_mountain = self.tile_adjacent_to(tile, Terrain::Mountain);
                        if near_mountain || self.tiles[tile].terrain == Terrain::Mountain {
                            let year = self.year;
                            let already = self.settlements[si].traits;
                            if already & TRAIT_MASONRY == 0 {
                                self.settlements[si].traits |= TRAIT_MASONRY | TRAIT_COLD_RESIST;
                                self.log_event(format!("Y{} mountain folk raise stone walls", year));
                            }
                        }
                    }
                }
                // Effect 2: ash raises K for neighbours (delayed 6..14 years)
                let delay = 6 + (self.roll() * 8.0) as u32;
                let delta = 0.15 + self.roll() * 0.20; // 0.15..0.35
                self.enqueue_soil_enrichment(epicentre, delta, delay);
                self.log_event(format!("Y{} a volcano buries the region", year));
            }

            DisasterKind::Quake => {
                // Effect 1: damage settlements in radius 1
                let quake_tiles: Vec<usize> = std::iter::once(epicentre)
                    .chain(self.neighbours(epicentre).iter().copied())
                    .collect();
                for &ni in &quake_tiles {
                    let si = self.occupied[ni];
                    if si >= 0 {
                        let survive_frac = 0.3 + self.roll() * 0.55;
                        self.settlements[si as usize].pop *= survive_frac;
                    }
                }
                // Effect 2: rubble raises K slightly (delayed 8..14 years)
                let delay = 8 + (self.roll() * 6.0) as u32;
                self.enqueue_soil_enrichment(epicentre, 0.10, delay);
                self.log_event(format!("Y{} the earth splits beneath the mountains", year));
            }

            DisasterKind::Flood => {
                // Effect 1: pop loss on epicentre + neighbours
                // Collect neighbours first to avoid simultaneous borrow.
                let flood_tiles: Vec<usize> = std::iter::once(epicentre)
                    .chain(self.neighbours(epicentre).iter().copied())
                    .collect();
                let mut flood_hits: Vec<(usize, f32)> = Vec::new();
                for ni in &flood_tiles {
                    let si = self.occupied[*ni];
                    if si >= 0 {
                        let survive_frac = 0.55 + self.roll() * 0.35;
                        flood_hits.push((si as usize, survive_frac));
                    }
                }
                for (si, survive_frac) in flood_hits {
                    // §4 PARKS: FLOOD_RESILIENCE reduces damage
                    let resilience_mult = if self.settlements[si].traits & TRAIT_FLOOD_RESILIENCE != 0 {
                        // resilient settlements lose less pop
                        survive_frac + (1.0 - survive_frac) * 0.45
                    } else {
                        survive_frac
                    };
                    self.settlements[si].pop *= resilience_mult;
                    // §4 PARKS: survived flood → earn FLOOD_RESILIENCE
                    self.settlements[si].disasters_survived += 1;
                    if self.settlements[si].traits & TRAIT_FLOOD_RESILIENCE == 0 {
                        self.settlements[si].traits |= TRAIT_FLOOD_RESILIENCE;
                        let yr = self.year;
                        self.log_event(format!("Y{} a people masters the floods", yr));
                    }
                }
                // Effect 2: alluvium raises flood-plain K (faster, 3..8 years)
                let delay = 3 + (self.roll() * 5.0) as u32;
                let delta = 0.10 + self.roll() * 0.15; // 0.10..0.25
                self.enqueue_soil_enrichment(epicentre, delta, delay);
                self.log_event(format!("Y{} floods drown the river towns", year));
            }

            DisasterKind::Drought => {
                // Effect 1: pop loss across epicentre + adjacent cluster
                let drought_tiles: Vec<usize> = std::iter::once(epicentre)
                    .chain(self.neighbours(epicentre).iter().copied())
                    .collect();
                for &ni in &drought_tiles {
                    let si = self.occupied[ni];
                    if si >= 0 {
                        let survive_frac = 0.5 + self.roll() * 0.30;
                        self.settlements[si as usize].pop *= survive_frac;
                    }
                }
                // Effect 2: negative K dip (temporary, 4..8 years)
                let dip_years = 4 + (self.roll() * 4.0) as u32;
                for &ni in &drought_tiles {
                    let t = self.tiles[ni].terrain;
                    if t.is_water() || t == Terrain::Mountain {
                        continue;
                    }
                    // Compute deltas before borrowing recovery to satisfy borrow checker.
                    let dip_delta = -(0.15 + self.roll() * 0.10) * K_MAX;
                    let rebound_delta = (0.08 + self.roll() * 0.07) * K_MAX;
                    // negative dip (years_left=1 so process_recovery decrements
                    // to 0 next tick and applies it)
                    self.recovery.push(KEdit {
                        tile: ni,
                        delta: dip_delta,
                        years_left: 1,
                    });
                    // followed by a smaller recovery (positive rebound)
                    self.recovery.push(KEdit {
                        tile: ni,
                        delta: rebound_delta,
                        years_left: dip_years + 1,
                    });
                }
                self.log_event(format!(
                    "Y{} drought cracks the desert-edge fields",
                    year
                ));
            }

            DisasterKind::Plague => {
                // Effect 1: pop devastation on highest-density settlement
                let si = self.occupied[epicentre];
                if si >= 0 {
                    let survive_frac = 0.35 + self.roll() * 0.25; // 0.35..0.60
                    self.settlements[si as usize].pop *= survive_frac;
                }
                // Spreads at reduced severity to NEIGHBORS8
                let nbrs = self.neighbours(epicentre);
                for ni in nbrs {
                    let nsi = self.occupied[ni];
                    if nsi >= 0 {
                        let spread_frac = 0.65 + self.roll() * 0.25;
                        self.settlements[nsi as usize].pop *= spread_frac;
                    }
                }
                // Record the afflicted nation so plague can later travel trade roads.
                let si2 = self.occupied[epicentre];
                if si2 >= 0 {
                    let nid = self.nation_of.get(si2 as usize).copied().unwrap_or(-1);
                    if nid >= 0 {
                        let nid_u = nid as u32;
                        self.plague_nations.entry(nid_u).or_insert(year);
                    }
                }
                self.log_event(format!("Y{} plague empties the crowded city", year));
            }

            DisasterKind::Tsunami => {
                // Collect the epicentre + immediate coastal neighbours (non-water
                // tiles adjacent to Ocean/DeepOcean) to keep the radius bounded.
                let tsunami_tiles: Vec<usize> = {
                    let mut v = vec![epicentre];
                    let col = epicentre % COLS;
                    let row = epicentre / COLS;
                    for &(dc, dr) in NEIGHBORS8.iter() {
                        let nc = col as i32 + dc;
                        let nr = row as i32 + dr;
                        if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                            continue;
                        }
                        let ni = idx(nc as usize, nr as usize);
                        // Include coastal land tiles in the flood radius.
                        let t = self.tiles[ni].terrain;
                        if !t.is_water() && self.tile_is_coastal(ni) {
                            v.push(ni);
                        }
                    }
                    v
                };

                // Collect hits to avoid multiple borrow during mutation.
                let mut tsunami_hits: Vec<(usize, bool)> = Vec::new(); // (si, direct_hit)
                for &ni in &tsunami_tiles {
                    let si = self.occupied[ni];
                    if si >= 0 {
                        let direct = ni == epicentre;
                        tsunami_hits.push((si as usize, direct));
                    }
                }

                for (si, direct) in tsunami_hits {
                    if direct {
                        // Direct hit: force-migrate survivors.
                        let tile = self.settlements[si].tile;
                        self.settlements[si].pop *= 0.25; // heavy casualties first
                        self.force_migrate(tile);
                    } else {
                        // Coastal neighbour: heavy pop damage but settlement may survive.
                        let survive_frac = 0.35 + self.roll() * 0.30; // 0.35..0.65
                        self.settlements[si].pop *= survive_frac;
                        self.settlements[si].disasters_survived += 1;
                    }
                }

                // Temporary K reduction on all flooded coastal tiles (recovers after 8..16 yrs).
                for &ni in &tsunami_tiles {
                    let t = self.tiles[ni].terrain;
                    if t.is_water() || t == Terrain::Mountain {
                        continue;
                    }
                    let dip_years = 8 + (self.roll() * 8.0) as u32;
                    let dip_delta = -(0.18 + self.roll() * 0.12) * K_MAX;
                    let rebound_delta = (0.10 + self.roll() * 0.08) * K_MAX;
                    // Immediate K dip (applied next recovery tick).
                    self.recovery.push(KEdit {
                        tile: ni,
                        delta: dip_delta,
                        years_left: 1,
                    });
                    // Gradual K recovery.
                    self.recovery.push(KEdit {
                        tile: ni,
                        delta: rebound_delta,
                        years_left: dip_years + 1,
                    });
                }

                self.log_event(format!("Y{} a tsunami swallows the coast", year));
            }
        }
    }

    /// Checks each disaster type (weighted by terrain) and fires those whose
    /// random roll beats the base chance.  Called once per year before growth.
    pub(crate) fn step_disasters(&mut self) {
        // 1. process the recovery queue first so K edits land before logistic
        //    growth reads K this tick.
        self.process_recovery();

        let count = COLS * ROWS;

        // 2. Volcano — pick a random Mountain tile
        if self.roll() < VOLCANO_CHANCE {
            let cands: Vec<usize> = (0..count)
                .filter(|&i| self.tiles[i].terrain == Terrain::Mountain)
                .collect();
            if let Some(epi) = self.pick_from(&cands) {
                self.apply_disaster(DisasterKind::Volcano, epi);
            }
        }

        // 3. Earthquake — mountain-adjacent habitable tile
        if self.roll() < QUAKE_CHANCE {
            let cands: Vec<usize> = (0..count)
                .filter(|&i| {
                    self.tiles[i].habitable
                        && self.tile_adjacent_to(i, Terrain::Mountain)
                })
                .collect();
            if let Some(epi) = self.pick_from(&cands) {
                self.apply_disaster(DisasterKind::Quake, epi);
            }
        }

        // 4. Flood — River tile or low Plains adjacent to river
        if self.roll() < FLOOD_CHANCE {
            let cands: Vec<usize> = (0..count)
                .filter(|&i| {
                    let t = self.tiles[i].terrain;
                    t == Terrain::River
                        || (t == Terrain::Plains
                            && self.tile_adjacent_to(i, Terrain::River))
                })
                .collect();
            if let Some(epi) = self.pick_from(&cands) {
                self.apply_disaster(DisasterKind::Flood, epi);
            }
        }

        // 5. Drought — habitable tile adjacent to Desert
        if self.roll() < DROUGHT_CHANCE {
            let cands: Vec<usize> = (0..count)
                .filter(|&i| {
                    self.tiles[i].habitable
                        && self.tile_adjacent_to(i, Terrain::Desert)
                })
                .collect();
            if let Some(epi) = self.pick_from(&cands) {
                self.apply_disaster(DisasterKind::Drought, epi);
            }
        }

        // 6. Plague — highest pop-density settlement (terrain-independent)
        if self.roll() < PLAGUE_CHANCE {
            let best = self
                .settlements
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    let k = self.tiles[s.tile].k;
                    k > 0.0 && s.pop > 0.0
                })
                .max_by(|(_, a), (_, b)| {
                    let da = a.pop / self.tiles[a.tile].k;
                    let db = b.pop / self.tiles[b.tile].k;
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(_, s)| s.tile);
            if let Some(epi) = best {
                self.apply_disaster(DisasterKind::Plague, epi);
            }
        }

        // 7. Tsunami — strikes a random coastal land tile (non-water tile adjacent
        //    to Ocean or DeepOcean).  If no eligible tile exists, skip silently.
        if self.roll() < TSUNAMI_CHANCE {
            let count = COLS * ROWS;
            let cands: Vec<usize> = (0..count)
                .filter(|&i| {
                    let t = self.tiles[i].terrain;
                    !t.is_water() && self.tile_is_coastal(i)
                })
                .collect();
            if let Some(epi) = self.pick_from(&cands) {
                self.apply_disaster(DisasterKind::Tsunami, epi);
            }
        }

        // 8. Wildfire — ignites a random Forest tile; spreads to adjacent Forest
        //    tiles up to WILDFIRE_MAX_SPREAD total (hard-capped BFS, never unbounded).
        //    Chance is higher when a drought struck recently.
        {
            let year = self.year;
            let near_drought = match self.last_disaster {
                Some((_, DisasterKind::Drought, yr)) => year.saturating_sub(yr) <= WILDFIRE_DROUGHT_WINDOW,
                _ => false,
            };
            let fire_chance = if near_drought { WILDFIRE_CHANCE_DROUGHT } else { WILDFIRE_CHANCE_BASE };

            if self.roll() < fire_chance {
                // Pick a random Forest tile as the ignition point.
                let forest_tiles: Vec<usize> = (0..COLS * ROWS)
                    .filter(|&i| self.tiles[i].terrain == Terrain::Forest)
                    .collect();

                if let Some(ignition) = self.pick_from(&forest_tiles) {
                    // BFS spread — hard-capped at WILDFIRE_MAX_SPREAD tiles total.
                    let mut visited: std::collections::HashSet<usize> = std::collections::HashSet::new();
                    let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
                    visited.insert(ignition);
                    queue.push_back(ignition);

                    let mut burned: Vec<usize> = Vec::new();

                    while let Some(current) = queue.pop_front() {
                        if burned.len() >= WILDFIRE_MAX_SPREAD {
                            break;
                        }
                        // Only burn Forest tiles.
                        if self.tiles[current].terrain != Terrain::Forest {
                            continue;
                        }
                        burned.push(current);

                        // Spread to Forest neighbours (if cap not yet reached).
                        if burned.len() < WILDFIRE_MAX_SPREAD {
                            let col = current % COLS;
                            let row = current / COLS;
                            for &(dc, dr) in NEIGHBORS8.iter() {
                                let nc = col as i32 + dc;
                                let nr = row as i32 + dr;
                                if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                                    continue;
                                }
                                let ni = idx(nc as usize, nr as usize);
                                if !visited.contains(&ni) && self.tiles[ni].terrain == Terrain::Forest {
                                    visited.insert(ni);
                                    queue.push_back(ni);
                                }
                            }
                        }
                    }

                    if !burned.is_empty() {
                        let scorch_until = year + WILDFIRE_SCORCH_YEARS;
                        for &tile in &burned {
                            // Scorch the tile so forest-burnt variant renders.
                            self.scorch[tile] = scorch_until;
                            // §8 flash the first burned tile.
                            if tile == ignition {
                                self.push_flash(tile);
                            }
                            // Damage/displace any settlement on the burned tile.
                            let si = self.occupied[tile];
                            if si >= 0 {
                                let si = si as usize;
                                // Reduce settlement population (fire damage).
                                self.settlements[si].pop *= 0.55;
                                // If the tile is now effectively uninhabitable due to
                                // scorching (K below threshold), force-migrate.
                                if !self.tiles[tile].habitable
                                    || self.tiles[tile].k < MIN_HABITABLE
                                {
                                    self.force_migrate(tile);
                                }
                            }
                        }
                        self.disaster_count += 1;
                        self.log_event(format!("Y{} wildfire sweeps the forest", year));
                    }
                }
            }
        }
    }
}
