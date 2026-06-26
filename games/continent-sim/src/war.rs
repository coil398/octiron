use crate::*;

// ---- War ------------------------------------------------------------------

/// An ongoing armed conflict between two nations.  `fronts` holds the tile
/// indices that are actively contested border tiles.
pub(crate) struct War {
    /// Nation id of the aggressor.
    pub(crate) a: u32,
    /// Nation id of the defender.
    pub(crate) b: u32,
    /// Contested border tile indices (settlement tiles of either side that are
    /// NEIGHBORS8-adjacent to the opposing nation).
    pub(crate) fronts: Vec<usize>,
    /// Years the war has been ongoing (feeds PARKS 'long war' → discipline).
    pub(crate) years: u32,
}

// ---- free functions -------------------------------------------------------

/// Terrain defense bonus added to the defender's local strength.
pub(crate) fn defense_bonus(terrain: Terrain) -> f32 {
    match terrain {
        Terrain::Mountain => 0.6,
        Terrain::River    => 0.4,
        Terrain::Hills    => 0.2,
        _                 => 0.0,
    }
}

// ---- impl Continent (war) -------------------------------------------------

impl Continent {
    /// Computes the military power of the nation at index `ni` (not id).
    /// Power = total population * tech multiplier * trait multiplier.
    pub(crate) fn national_power_by_idx(&self, ni: usize) -> f32 {
        let nation = &self.nations[ni];
        let pop_sum: f32 = nation
            .territory
            .iter()
            .map(|&si| self.settlements[si].pop)
            .sum();
        let tech_mult = 1.0
            + if nation.tech & TECH_METALLURGY != 0 { 0.20 } else { 0.0 }
            + if nation.tech & TECH_WRITING != 0 { 0.10 } else { 0.0 };
        // §4 PARKS: DISCIPLINE adds military power; MASONRY adds defense (applied at front).
        let trait_mult = 1.0
            + if nation.traits & TRAIT_DISCIPLINE != 0 { 0.15 } else { 0.0 };
        pop_sum * tech_mult * trait_mult
    }

    /// Chebyshev tile-grid distance between two tile indices.
    pub(crate) fn grid_dist(a: usize, b: usize) -> i32 {
        let ac = (a % COLS) as i32;
        let ar = (a / COLS) as i32;
        let bc = (b % COLS) as i32;
        let br = (b / COLS) as i32;
        (ac - bc).abs().max((ar - br).abs())
    }

    /// Returns a supply factor in `0.0..=1.0` for a border tile given the
    /// OWNING nation's capital: decays with grid distance.
    pub(crate) fn supply_factor(capital_tile: usize, border_tile: usize) -> f32 {
        let d = Continent::grid_dist(capital_tile, border_tile) as f32;
        (1.0 - d * SUPPLY_DECAY).clamp(0.10, 1.0)
    }

    /// Returns the index of the nation with the given id, or `None`.
    pub(crate) fn nation_idx_by_id(&self, id: u32) -> Option<usize> {
        self.nations.iter().position(|n| n.id == id)
    }

    /// Builds the set of contested border tiles between two nations identified
    /// by their indices.  A border tile is any settlement tile of nation A that
    /// is NEIGHBORS8-adjacent to a settlement tile of nation B (or vice-versa).
    pub(crate) fn collect_front_tiles(&self, ni_a: usize, ni_b: usize) -> Vec<usize> {
        // Collect tile sets for both nations.
        let tiles_b: std::collections::HashSet<usize> = self.nations[ni_b]
            .territory
            .iter()
            .map(|&si| self.settlements[si].tile)
            .collect();

        let mut fronts = std::collections::HashSet::new();

        for &si in &self.nations[ni_a].territory {
            let tile = self.settlements[si].tile;
            let col = tile % COLS;
            let row = tile / COLS;
            for &(dc, dr) in NEIGHBORS8.iter() {
                let nc = col as i32 + dc;
                let nr = row as i32 + dr;
                if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                    continue;
                }
                let ni = idx(nc as usize, nr as usize);
                if tiles_b.contains(&ni) {
                    // The attacker's tile is the front.
                    fronts.insert(tile);
                }
            }
        }

        // Sort so that iteration order is deterministic across processes
        // (HashSet order is undefined; we iterate front_tiles with roll() calls).
        let mut v: Vec<usize> = fronts.into_iter().collect();
        v.sort_unstable();
        v
    }

    /// §9 War: detect borders, possibly start wars, resolve front tiles.
    /// Runs every WAR_CADENCE years alongside step_nations.
    pub(crate) fn step_war(&mut self) {
        let year = self.year;
        let n_nations = self.nations.len();
        if n_nations < 2 {
            return;
        }

        let mut log_msgs: Vec<String> = Vec::new();

        // ---- 1. AGE existing wars -------------------------------------------
        // Take wars out so we can call &self methods freely below.
        let mut wars = std::mem::take(&mut self.wars);
        for w in &mut wars {
            w.years += WAR_CADENCE;
        }

        // ---- 2. DETECT new wars between bordering nations -------------------
        let existing_pairs: std::collections::HashSet<(u32, u32)> = wars
            .iter()
            .map(|w| (w.a.min(w.b), w.a.max(w.b)))
            .collect();

        // Build a list of (ni_a, ni_b) that border each other.
        let mut border_pairs: Vec<(usize, usize)> = Vec::new();
        for na_idx in 0..n_nations {
            let tiles_a: std::collections::HashSet<usize> = self.nations[na_idx]
                .territory
                .iter()
                .map(|&si| self.settlements[si].tile)
                .collect();

            for nb_idx in (na_idx + 1)..n_nations {
                let mut adjacent = false;
                'outer: for &si in &self.nations[nb_idx].territory {
                    let tile = self.settlements[si].tile;
                    let col = tile % COLS;
                    let row = tile / COLS;
                    for &(dc, dr) in NEIGHBORS8.iter() {
                        let nc = col as i32 + dc;
                        let nr = row as i32 + dr;
                        if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                            continue;
                        }
                        if tiles_a.contains(&idx(nc as usize, nr as usize)) {
                            adjacent = true;
                            break 'outer;
                        }
                    }
                }
                if adjacent {
                    border_pairs.push((na_idx, nb_idx));
                }
            }
        }

        for (na_idx, nb_idx) in &border_pairs {
            let id_a = self.nations[*na_idx].id;
            let id_b = self.nations[*nb_idx].id;
            let pair = (id_a.min(id_b), id_a.max(id_b));
            if existing_pairs.contains(&pair) {
                continue;
            }

            // §9 Alliances: two nations are allied when they share the same
            // culture AND are current trade partners (border-adjacent and not at
            // war — both conditions are already satisfied here because we are
            // inside the border_pairs loop and the pair is not in existing_pairs).
            // Allied nations may never initiate war with each other.
            let same_culture = self.nations[*na_idx].culture == self.nations[*nb_idx].culture;
            if same_culture {
                // Log an alliance peace message occasionally (throttled: ~1 in 50
                // checks on average, using the sim RNG so it is deterministic and
                // bounded — never fires more than once per WAR_CADENCE window per pair).
                if self.roll() < 0.02 {
                    let name_a = self.nations[*na_idx].name.clone();
                    let name_b = self.nations[*nb_idx].name.clone();
                    log_msgs.push(format!(
                        "Y{} an alliance keeps the peace between {} and {}",
                        year, name_a, name_b
                    ));
                    self.alliance_count += 1;
                }
                continue; // allied — war is forbidden
            }

            let pow_a = self.national_power_by_idx(*na_idx);
            let pow_b = self.national_power_by_idx(*nb_idx);
            let imbalance = if pow_a + pow_b > 0.0 {
                (pow_a - pow_b).abs() / (pow_a + pow_b)
            } else {
                0.0
            };
            let base_chance = WAR_START_CHANCE * (1.0 + imbalance);
            // Scale by the aggressor (higher-power side) polity.
            let aggressor_polity = if pow_a >= pow_b {
                self.nations[*na_idx].polity
            } else {
                self.nations[*nb_idx].polity
            };
            let polity_scale = match aggressor_polity {
                Polity::Militarist   => 1.5,
                Polity::Merchant     => 0.6,
                Polity::Isolationist => 0.5,
                _                    => 1.0,
            };
            let chance = base_chance * polity_scale;
            if self.roll() < chance {
                let fronts = self.collect_front_tiles(*na_idx, *nb_idx);
                if !fronts.is_empty() {
                    let name_a = self.nations[*na_idx].name.clone();
                    let name_b = self.nations[*nb_idx].name.clone();
                    wars.push(War { a: id_a, b: id_b, fronts, years: 0 });
                    log_msgs.push(format!(
                        "Y{} war breaks out between {} and {}",
                        year, name_a, name_b
                    ));
                }
            }
        }

        // ---- 3. RESOLVE each front tile in each active war ------------------
        // We accumulate (target_si, new_nation_id, attacker_si) and also the
        // log messages so we can call self.roll() without borrow conflicts.
        // Each tuple: (target_si, attacker_si, new_nation_id, log_msg, log_repel_msg)
        struct FrontResult {
            target_si: usize,
            attacker_si: usize,
            new_nation_id: u32,
            log_msg: Option<String>,
            log_repel_msg: Option<String>,
        }
        let mut front_results: Vec<FrontResult> = Vec::new();

        for wi in 0..wars.len() {
            let war_a = wars[wi].a;
            let war_b = wars[wi].b;
            let ni_a = match self.nations.iter().position(|n| n.id == war_a) {
                Some(i) => i,
                None => continue,
            };
            let ni_b = match self.nations.iter().position(|n| n.id == war_b) {
                Some(i) => i,
                None => continue,
            };

            let pow_a = self.national_power_by_idx(ni_a);
            let pow_b = self.national_power_by_idx(ni_b);
            let cap_a = self.nations[ni_a].capital;
            let cap_b = self.nations[ni_b].capital;

            // Refresh front tiles.
            let fresh_fronts = self.collect_front_tiles(ni_a, ni_b);
            wars[wi].fronts = fresh_fronts;

            let front_tiles = wars[wi].fronts.clone();
            for &front_tile in &front_tiles {
                let si_at_front = self.occupied[front_tile];
                if si_at_front < 0 {
                    continue;
                }
                let si_at_front = si_at_front as usize;
                let tile_nation_id = self.nation_of.get(si_at_front).copied().unwrap_or(-1);
                if tile_nation_id < 0 {
                    continue;
                }
                let tile_nation_id = tile_nation_id as u32;

                let (atk_pow, atk_nation_id, def_cap) = if tile_nation_id == war_a {
                    (pow_a, war_a, cap_b)
                } else if tile_nation_id == war_b {
                    (pow_b, war_b, cap_a)
                } else {
                    continue;
                };
                let def_nation_id = if atk_nation_id == war_a { war_b } else { war_a };

                // Find the adjacent enemy settlement.
                let col = front_tile % COLS;
                let row = front_tile / COLS;
                let mut target_si: Option<usize> = None;
                for &(dc, dr) in NEIGHBORS8.iter() {
                    let nc = col as i32 + dc;
                    let nr = row as i32 + dr;
                    if nc < 0 || nr < 0 || nc >= COLS as i32 || nr >= ROWS as i32 {
                        continue;
                    }
                    let nidx_t = idx(nc as usize, nr as usize);
                    let nsi = self.occupied[nidx_t];
                    if nsi < 0 {
                        continue;
                    }
                    let nsi = nsi as usize;
                    if self.nation_of.get(nsi).copied().unwrap_or(-1) == def_nation_id as i32 {
                        target_si = Some(nsi);
                        break;
                    }
                }
                let target_si = match target_si { Some(t) => t, None => continue };

                let target_tile = self.settlements[target_si].tile;
                let terrain = self.tiles[target_tile].terrain;
                // §4 PARKS: MASONRY + DISCIPLINE add to the defender's terrain bonus.
                let def_trait_bonus = Continent::trait_defense_bonus(self.settlements[target_si].traits);
                let def_local = self.settlements[target_si].pop
                    * (1.0 + defense_bonus(terrain) + def_trait_bonus)
                    * Continent::supply_factor(def_cap, target_tile);

                let atk_tech = self.nations[if atk_nation_id == war_a { ni_a } else { ni_b }].tech;
                let def_tech = self.nations[if atk_nation_id == war_a { ni_b } else { ni_a }].tech;
                let tech_gap = (atk_tech.count_ones() as i32 - def_tech.count_ones() as i32)
                    .max(0) as f32;
                let advantage = atk_pow - def_local + tech_gap * 50.0;
                let advance_prob = (advantage * ADVANCE_RATE / atk_pow.max(1.0)).clamp(0.0, 0.85);

                let roll1 = self.roll();
                let roll2 = self.roll();

                let (ni_atk, ni_def) = if atk_nation_id == war_a { (ni_a, ni_b) } else { (ni_b, ni_a) };
                let atk_name = self.nations[ni_atk].name.clone();
                let def_name = self.nations[ni_def].name.clone();

                if roll1 < advance_prob {
                    front_results.push(FrontResult {
                        target_si,
                        attacker_si: si_at_front,
                        new_nation_id: atk_nation_id,
                        log_msg: Some(format!(
                            "Y{} {} seizes a border town from {}",
                            year, atk_name, def_name
                        )),
                        log_repel_msg: None,
                    });
                } else if defense_bonus(terrain) >= 0.4 && roll2 < 0.30 {
                    front_results.push(FrontResult {
                        target_si,
                        attacker_si: si_at_front,
                        new_nation_id: def_nation_id, // no transfer
                        log_msg: None,
                        log_repel_msg: Some(format!(
                            "Y{} the terrain holds; {} repels the assault",
                            year, def_name
                        )),
                    });
                }
            }
        }

        // Put wars back before applying transfers (transfer code reads self.wars
        // indirectly via nation_idx_by_id / territory, not via wars vec).
        self.wars = wars;

        // ---- 4. APPLY tile transfers -----------------------------------------
        let mut collapse_nation_ids: Vec<u32> = Vec::new();
        for fr in &front_results {
            if let Some(ref msg) = fr.log_msg {
                log_msgs.push(msg.clone());
            }
            if let Some(ref msg) = fr.log_repel_msg {
                log_msgs.push(msg.clone());
                continue; // no transfer for a repel
            }
            let old_nation_id = self.nation_of.get(fr.target_si).copied().unwrap_or(-1);
            // Skip if this settlement already changed owner this tick (e.g. two of
            // the attacker's tiles border it) BEFORE applying pop loss — otherwise
            // the town would be double-attrited with only one real transfer.
            if old_nation_id < 0 || old_nation_id as u32 == fr.new_nation_id {
                continue;
            }
            let old_nation_id = old_nation_id as u32;
            // Pop loss (only on an actual transfer).
            self.settlements[fr.target_si].pop *= WAR_POP_LOSS_DEFENDER;
            self.settlements[fr.attacker_si].pop *= WAR_POP_LOSS_ATTACKER;
            self.nation_of[fr.target_si] = fr.new_nation_id as i32;

            // Remove from old nation.
            if let Some(old_ni) = self.nation_idx_by_id(old_nation_id) {
                self.nations[old_ni].territory.retain(|&s| s != fr.target_si);
            }
            // Add to new nation.
            if let Some(new_ni) = self.nation_idx_by_id(fr.new_nation_id) {
                if !self.nations[new_ni].territory.contains(&fr.target_si) {
                    self.nations[new_ni].territory.push(fr.target_si);
                }
            }
            // Check defender collapse.
            if let Some(def_ni) = self.nation_idx_by_id(old_nation_id) {
                let cap_tile = self.nations[def_ni].capital;
                let cap_lost = self.settlements[fr.target_si].tile == cap_tile;
                if cap_lost || self.nations[def_ni].territory.is_empty() {
                    collapse_nation_ids.push(old_nation_id);
                }
            }
        }

        // ---- 5. COLLAPSE: remove nations with no territory / lost capital ----
        collapse_nation_ids.dedup();
        for col_id in collapse_nation_ids {
            if let Some(col_ni) = self.nation_idx_by_id(col_id) {
                let col_name = self.nations[col_ni].name.clone();
                let atk_name = self
                    .wars
                    .iter()
                    .find(|w| w.a == col_id || w.b == col_id)
                    .map(|w| {
                        let atk_id = if w.a == col_id { w.b } else { w.a };
                        self.nation_idx_by_id(atk_id)
                            .map(|ni| self.nations[ni].name.clone())
                            .unwrap_or_else(|| "an enemy".to_string())
                    })
                    .unwrap_or_else(|| "an enemy".to_string());
                log_msgs.push(format!(
                    "Y{} {} collapses before {}",
                    year, col_name, atk_name
                ));
                self.nations.remove(col_ni);
                // Orphan its remaining settlements so no nation_of holds a dead id.
                for nid in self.nation_of.iter_mut() {
                    if *nid == col_id as i32 {
                        *nid = -1;
                    }
                }
            }
        }

        // ---- 6. END wars whose fronts are empty or one side is gone ---------
        let nation_ids: std::collections::HashSet<u32> =
            self.nations.iter().map(|n| n.id).collect();
        let mut ended_wars: Vec<(String, String, u32)> = Vec::new();
        self.wars.retain(|w| {
            let both_live = nation_ids.contains(&w.a) && nation_ids.contains(&w.b);
            if !both_live || w.fronts.is_empty() {
                let na = self.nations.iter().find(|n| n.id == w.a)
                    .map(|n| n.name.clone()).unwrap_or_else(|| "?".to_string());
                let nb = self.nations.iter().find(|n| n.id == w.b)
                    .map(|n| n.name.clone()).unwrap_or_else(|| "?".to_string());
                ended_wars.push((na, nb, w.years));
                false
            } else {
                true
            }
        });
        for (na, nb, yrs) in ended_wars {
            log_msgs.push(format!(
                "Y{} peace returns after {} years ({} / {})",
                year, yrs, na, nb
            ));
            // §9 War weariness: if the war was long, log exhaustion once per
            // nation (both sides) at resolution time.
            if yrs >= WAR_WEARINESS_THRESHOLD {
                log_msgs.push(format!("Y{} {} is exhausted by the long war", year, na));
                log_msgs.push(format!("Y{} {} is exhausted by the long war", year, nb));
            }
        }

        // ---- 7. Emit all log messages ----------------------------------------
        for msg in log_msgs {
            self.log_event(msg);
        }
    }
}
