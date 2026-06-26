use crate::*;

// ---- era system -----------------------------------------------------------

/// Global era / age, derived from the count of world-first tech discoveries.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Era {
    Stone,
    Neolithic,
    Ancient,
    Medieval,
    Modern,
}

impl Era {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Era::Stone     => "Stone Age",
            Era::Neolithic => "Neolithic",
            Era::Ancient   => "Ancient",
            Era::Medieval  => "Medieval",
            Era::Modern    => "Modern",
        }
    }

    pub(crate) fn label_jp(self) -> &'static str {
        match self {
            Era::Stone     => "石器時代",
            Era::Neolithic => "新石器時代",
            Era::Ancient   => "古代",
            Era::Medieval  => "中世",
            Era::Modern    => "近代",
        }
    }

    /// Maps tech count (0-based number of bits set in `tech_unlocked`) to an era.
    /// 0 → Stone, 1 → Neolithic, 2 → Ancient, 3 → Medieval, >=4 → Modern.
    pub(crate) fn from_tech_count(n: u32) -> Era {
        match n {
            0 => Era::Stone,
            1 => Era::Neolithic,
            2 => Era::Ancient,
            3 => Era::Medieval,
            _ => Era::Modern,
        }
    }

    /// Era index (0..=4) used for numeric effects (e.g. growth scaling).
    pub(crate) fn index(self) -> u32 {
        match self {
            Era::Stone     => 0,
            Era::Neolithic => 1,
            Era::Ancient   => 2,
            Era::Medieval  => 3,
            Era::Modern    => 4,
        }
    }
}

// ---- traits display -------------------------------------------------------

/// Returns a short string listing earned traits (e.g. "F M - - - D").
pub(crate) fn traits_display(traits: u32) -> String {
    let f = if traits & TRAIT_FLOOD_RESILIENCE != 0 { "F" } else { "-" };
    let m = if traits & TRAIT_MASONRY != 0 { "M" } else { "-" };
    let c = if traits & TRAIT_COLD_RESIST != 0 { "C" } else { "-" };
    let s = if traits & TRAIT_SEAFARING_TRAIT != 0 { "S" } else { "-" };
    let t = if traits & TRAIT_COMMERCE != 0 { "T" } else { "-" };
    let d = if traits & TRAIT_DISCIPLINE != 0 { "D" } else { "-" };
    format!("{} {} {} {} {} {}", f, m, c, s, t, d)
}

// ---- impl Continent (technology / parks / era / milestones) ---------------

impl Continent {
    /// Derives the current global era from the world-first tech bitmask.
    pub(crate) fn current_era(&self) -> Era {
        Era::from_tech_count(self.tech_unlocked.count_ones())
    }

    /// §3 Technology: trigger-fired by pressure × locale, never chosen.
    /// Called in tick() AFTER expansion so stuck/at-cap state is current.
    pub(crate) fn step_tech(&mut self) {
        let n = self.settlements.len();
        let year = self.year;

        // We collect the tech grants before applying them to avoid borrow
        // conflicts between settlement fields and tile reads.
        // Each entry: (settlement index, tech bit, optional K-boost tile)
        let mut grants: Vec<(usize, u8, Option<(usize, f32)>)> = Vec::new();

        for i in 0..n {
            let tile = self.settlements[i].tile;
            let pop = self.settlements[i].pop;
            let k = self.tiles[tile].k;
            let at_pressure = k > 0.0 && pop / k >= PRESSURE;

            // -- IRRIGATION --------------------------------------------------
            // Trigger: river-adjacent + at K-cap for IRRIGATION_THRESHOLD years.
            if self.settlements[i].tech & TECH_IRRIGATION == 0 {
                if at_pressure && self.tile_near_river(tile) {
                    self.settlements[i].at_cap_years += 1;
                    if self.settlements[i].at_cap_years >= IRRIGATION_THRESHOLD {
                        grants.push((
                            i,
                            TECH_IRRIGATION,
                            Some((tile, IRRIGATION_K_BOOST)),
                        ));
                    }
                } else if !at_pressure {
                    // Reset counter when pressure drops.
                    self.settlements[i].at_cap_years = 0;
                }
            }

            // -- SEAFARING ---------------------------------------------------
            // Trigger: coastal + stuck >= SEAFARING_THRESHOLD at pressure.
            if self.settlements[i].tech & TECH_SEAFARING == 0 {
                if at_pressure
                    && self.settlements[i].stuck >= SEAFARING_THRESHOLD
                    && self.tile_is_coastal(tile)
                {
                    grants.push((i, TECH_SEAFARING, None));
                }
            }
            // §4 PARKS: sustained coastal trade pressure → SEAFARING_TRAIT + COMMERCE
            // (independent of the tech system; fires when coastal + prolonged stuck)
            if self.settlements[i].traits & TRAIT_COMMERCE == 0
                && self.tile_is_coastal(tile)
                && self.settlements[i].stuck >= SEAFARING_THRESHOLD + 3
            {
                // Defer to grants vec to avoid borrow conflict.
                grants.push((i, 0, None)); // marker: bit=0 = parks commerce grant
            }

            // -- METALLURGY --------------------------------------------------
            // Trigger 1: mountain neighbour + surplus (pop > 0.9 * k).
            if self.settlements[i].tech & TECH_METALLURGY == 0 {
                if k > 0.0
                    && pop > 0.9 * k
                    && self.tile_adjacent_to(tile, Terrain::Mountain)
                {
                    grants.push((
                        i,
                        TECH_METALLURGY,
                        Some((tile, METALLURGY_K_BOOST)),
                    ));
                }
            }
            // Trigger 2 (resource hook): settlement sitting on an Iron tile gets
            // TECH_METALLURGY immediately (simplest correct hook — resources layer).
            if self.settlements[i].tech & TECH_METALLURGY == 0 {
                if tile < self.resources.len() && self.resources[tile] == Resource::Iron {
                    grants.push((i, TECH_METALLURGY, Some((tile, METALLURGY_K_BOOST))));
                }
            }

            // -- WRITING / BUREAUCRACY ---------------------------------------
            // Trigger: settlement size threshold.
            if self.settlements[i].tech & TECH_WRITING == 0 {
                if pop >= WRITING_POP_THRESHOLD {
                    grants.push((i, TECH_WRITING, None));
                }
            }
        }

        // Apply grants and emit first-discovery chronicle entries.
        for (si, bit, k_boost) in grants {
            // bit == 0 is a PARKS commerce marker (not a tech grant).
            if bit == 0 {
                if self.settlements[si].traits & TRAIT_COMMERCE == 0 {
                    self.settlements[si].traits |= TRAIT_SEAFARING_TRAIT | TRAIT_COMMERCE;
                    self.log_event(format!("Y{} {} grows rich on trade", year, "a coastal people"));
                }
                continue;
            }

            self.settlements[si].tech |= bit;

            // K boost (irrigation / metallurgy): raise cap, recompute habitability.
            if let Some((tile, frac)) = k_boost {
                let new_k = (self.tiles[tile].k + frac * K_MAX).clamp(0.0, K_MAX * 1.4);
                self.tiles[tile].k = new_k;
                let t = self.tiles[tile].terrain;
                let now_hab = !t.is_water() && t != Terrain::Mountain && new_k >= MIN_HABITABLE;
                let was_hab = self.tiles[tile].habitable;
                self.tiles[tile].habitable = now_hab;
                if now_hab && !was_hab {
                    self.habitable_count += 1;
                }
                // Reset the at_cap counter so irrigation can theoretically fire
                // again on a different settlement later.
                if bit == TECH_IRRIGATION {
                    self.settlements[si].at_cap_years = 0;
                }
            }

            // First-discovery chronicle (gated by global tech_unlocked bitmask).
            if self.tech_unlocked & bit == 0 {
                self.tech_unlocked |= bit;
                let msg = match bit {
                    TECH_IRRIGATION => format!("Y{} they learn to channel the river", year),
                    TECH_SEAFARING => format!("Y{} sailors set out across the sea", year),
                    TECH_METALLURGY => format!("Y{} smiths forge metal in the hills", year),
                    TECH_WRITING => format!("Y{} scribes begin to keep records", year),
                    _ => continue,
                };
                self.log_event(msg);
            }
        }
    }

    /// PARKS step: accumulate war_years on settlements that are in nations
    /// currently at war, then award DISCIPLINE to long-war nations; propagate
    /// settlement traits → nation.traits; persist to lineage_traits.
    /// Called once per tick (cheap — just bit-ops and small loops).
    pub(crate) fn step_parks(&mut self) {
        let year = self.year;

        // ---- 1. Accumulate war_years on settlements involved in active wars ---
        // Build the set of nation ids currently at war.
        let at_war_nids: std::collections::HashSet<u32> = self
            .wars
            .iter()
            .flat_map(|w| [w.a, w.b])
            .collect();

        for si in 0..self.settlements.len() {
            let nid = self.nation_of.get(si).copied().unwrap_or(-1);
            if nid >= 0 && at_war_nids.contains(&(nid as u32)) {
                self.settlements[si].war_years += 1;
            }
        }

        // ---- 2. Award DISCIPLINE to nations in wars lasting > threshold ------
        // Collect (nation_idx, earned) to avoid borrowing issues.
        let mut discipline_awards: Vec<(usize, bool)> = Vec::new();
        for (ni, nation) in self.nations.iter().enumerate() {
            if nation.traits & TRAIT_DISCIPLINE != 0 {
                continue; // already has it
            }
            // Check if this nation is in a long-running war.
            let in_long_war = self.wars.iter().any(|w| {
                (w.a == nation.id || w.b == nation.id) && w.years >= DISCIPLINE_WAR_THRESHOLD
            });
            if in_long_war {
                discipline_awards.push((ni, true));
            }
        }
        for (ni, _) in discipline_awards {
            self.nations[ni].traits |= TRAIT_DISCIPLINE;
            // Also grant discipline to all member settlements.
            let territory = self.nations[ni].territory.clone();
            for si in territory {
                self.settlements[si].traits |= TRAIT_DISCIPLINE;
            }
            let name = self.nations[ni].name.clone();
            self.log_event(format!("Y{} years of war forge a disciplined people in {}", year, name));
        }

        // ---- 3. Propagate settlement traits → nation.traits -----------------
        for nation in &mut self.nations {
            let union_traits = nation
                .territory
                .iter()
                .fold(0u32, |acc, &si| acc | self.settlements[si].traits);
            // Log newly-gained nation traits.
            let _new_traits = union_traits & !nation.traits;
            nation.traits = union_traits;
        }

        // ---- 4. Persist nation traits → lineage_traits ----------------------
        for nation in &self.nations {
            let entry = self.lineage_traits.entry(nation.id).or_insert(0);
            *entry |= nation.traits;
        }
    }

    /// Checks population milestones, era advances, unification, and extinction.
    /// Called at the end of each tick() so all state is current.
    pub(crate) fn step_milestones(&mut self) {
        while self.next_pop_milestone < COUNT_MILESTONES.len()
            && self.settlements.len() >= COUNT_MILESTONES[self.next_pop_milestone]
        {
            let m = COUNT_MILESTONES[self.next_pop_milestone];
            self.next_pop_milestone += 1;
            let year = self.year;
            self.log_event(format!("Y{} civilization spans {} towns", year, m));
        }

        // (a) Era advance: log when the era changes.
        let era_now = self.current_era();
        if era_now != self.prev_era {
            let year = self.year;
            self.log_event(format!("Y{} the world enters the {} age", year, era_now.label()));
            self.prev_era = era_now;
            self.era_changed = true;
        }

        // (b) Unification: one nation holds >= UNIFICATION_THRESHOLD of all settlements.
        if !self.unification_logged && !self.settlements.is_empty() {
            let threshold = (self.settlements.len() as f32 * UNIFICATION_THRESHOLD) as usize;
            let dominant = self.nations.iter().any(|n| n.territory.len() >= threshold);
            if dominant {
                self.unification_logged = true;
                let year = self.year;
                self.log_event(format!("Y{} one power dominates the known world", year));
            }
        }

        // (c) Extinction: all settlements gone after having been populated.
        if !self.extinction_logged && self.settlements.is_empty() && self.year > 0 {
            self.extinction_logged = true;
            let year = self.year;
            self.log_event(format!("Y{} the last people vanish from the world", year));
        }
    }
}
