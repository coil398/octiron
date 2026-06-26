use crate::*;

// ---- Polity ---------------------------------------------------------------

/// Polity type: summarises a nation's dominant strategic orientation, derived
/// each step from its earned traits and territory size.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Polity {
    Balanced,
    Militarist,
    Merchant,
    Isolationist,
    Expansionist,
}

// ---- Nation ---------------------------------------------------------------

/// A nation is a union of adjacent settlements that share enough cohesion.
/// Territory is stored as a list of settlement indices (into `Continent::settlements`).
pub(crate) struct Nation {
    pub(crate) id: u32,
    pub(crate) name: String,
    /// Tile index of the capital (highest-pop settlement at founding/election).
    pub(crate) capital: usize,
    /// Settlement indices belonging to this nation.
    pub(crate) territory: Vec<usize>,
    /// Union of all member settlements' tech flags.
    pub(crate) tech: u8,
    /// Union of all member settlements' trait bits (§4 PARKS — 0 until that system lands).
    pub(crate) traits: u32,
    /// Culture group id: derived from the capital tile's grid region at founding;
    /// inherited unchanged by daughter nations on split.  Nations sharing the same
    /// culture are less likely to go to war (war-start probability × 0.5).
    pub(crate) culture: u8,
    /// Low-alpha tint color used to paint territory on the map.
    pub(crate) color: Color,
    /// Current polity type, re-derived each step_nations pass.
    pub(crate) polity: Polity,
    /// Number of distinct peaceful trade partners (updated each nations-cadence step).
    pub(crate) trade_partners: u32,
    /// True while this nation is in a golden age (>= 2 trade partners, not at war).
    pub(crate) golden: bool,
}

// ---- free functions -------------------------------------------------------

/// Deterministically picks a nation tint color from an id.  Uses a small
/// palette of visually distinct hues so adjacent nations look different.
pub(crate) fn nation_color(id: u32) -> Color {
    const PALETTE: [[f32; 3]; 8] = [
        [0.80, 0.25, 0.25], // red
        [0.25, 0.55, 0.80], // blue
        [0.25, 0.75, 0.40], // green
        [0.85, 0.70, 0.15], // gold
        [0.75, 0.35, 0.80], // purple
        [0.85, 0.50, 0.15], // orange
        [0.20, 0.80, 0.80], // cyan
        [0.85, 0.30, 0.65], // pink
    ];
    let c = PALETTE[(id as usize) % PALETTE.len()];
    [c[0], c[1], c[2], 0.30]
}

// ---- impl Continent (nations / war / trade / parks) ----------------------

impl Continent {
    // ---- helper: pick/generate a nation name --------------------------------

    pub(crate) fn next_nation_name(&mut self) -> String {
        let base = NATION_NAMES[self.nation_name_idx % NATION_NAMES.len()];
        let suffix = self.nation_name_idx / NATION_NAMES.len();
        self.nation_name_idx += 1;
        if suffix == 0 {
            base.to_string()
        } else {
            format!("{} {}", base, suffix + 1)
        }
    }

    // ---- §2 nations system --------------------------------------------------

    /// Returns the grid-distance (Chebyshev / ∞-norm) between two tile indices.
    pub(crate) fn tile_dist(a: usize, b: usize) -> i32 {
        let ac = (a % COLS) as i32;
        let ar = (a / COLS) as i32;
        let bc = (b % COLS) as i32;
        let br = (b / COLS) as i32;
        (ac - bc).abs().max((ar - br).abs())
    }

    /// Computes the effective admin limit for a nation (raised by WRITING tech and Era).
    /// Era bonus lets an empire in later eras grow large enough to reach Unification.
    pub(crate) fn admin_limit(tech: u8, era: Era) -> usize {
        let writing_bonus = if tech & TECH_WRITING != 0 { ADMIN_WRITING_BONUS } else { 0 };
        let era_bonus = ADMIN_ERA_BONUS[era.index() as usize];
        ADMIN_BASE_LIMIT + writing_bonus + era_bonus
    }

    /// Returns the total population of a nation (sum of all member settlements).
    pub(crate) fn nation_total_pop(&self, nation: &Nation) -> f64 {
        nation
            .territory
            .iter()
            .filter_map(|&si| self.settlements.get(si))
            .map(|s| s.pop as f64)
            .sum()
    }

    /// Returns the settlement index with the highest population among the given
    /// settlement indices.
    pub(crate) fn highest_pop_settlement(&self, sids: &[usize]) -> Option<usize> {
        sids.iter().copied().max_by(|&a, &b| {
            self.settlements[a]
                .pop
                .partial_cmp(&self.settlements[b].pop)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Nations step: BFS flood-fill over occupied settlements, then unify /
    /// split based on cohesion and admin overload.  Runs every NATIONS_CADENCE
    /// years so it stays cheap.
    pub(crate) fn step_nations(&mut self) {
        let n_sett = self.settlements.len();
        if n_sett == 0 {
            return;
        }

        // Grow nation_of in case new settlements were spawned since last call.
        while self.nation_of.len() < n_sett {
            self.nation_of.push(-1);
        }

        // ---- 1. BUILD ADJACENCY among settlements ----------------------------
        // Two settlements are "adjacent" when their tile Chebyshev distance is
        // at most COHESION_DIST and neither tile is a blocking water/mountain
        // barrier between them (simplified: distance check only, matching the
        // design spec's union-find over NEIGHBORS8-reachable tiles).

        // Union-Find (path-compressed, by-rank abbreviated) over settlement indices.
        let mut parent: Vec<usize> = (0..n_sett).collect();

        fn find(parent: &mut Vec<usize>, mut x: usize) -> usize {
            while parent[x] != x {
                parent[x] = parent[parent[x]]; // path halving
                x = parent[x];
            }
            x
        }

        fn union(parent: &mut Vec<usize>, a: usize, b: usize) {
            let ra = find(parent, a);
            let rb = find(parent, b);
            if ra != rb {
                parent[rb] = ra;
            }
        }

        for i in 0..n_sett {
            for j in (i + 1)..n_sett {
                let dist = Continent::tile_dist(
                    self.settlements[i].tile,
                    self.settlements[j].tile,
                );
                if dist <= COHESION_DIST {
                    union(&mut parent, i, j);
                }
            }
        }

        // Collect connected components (root -> [settlement indices]).
        // Use BTreeMap so iteration is in ascending root-tile-index order,
        // giving next_nation_id / next_nation_name() / nation_color() a
        // deterministic assignment that is identical across runs for the same seed.
        let mut components: std::collections::BTreeMap<usize, Vec<usize>> =
            std::collections::BTreeMap::new();
        for i in 0..n_sett {
            let root = find(&mut parent, i);
            components.entry(root).or_default().push(i);
        }

        // ---- 2. UNIFY / FOUND new nations from components -------------------
        // For each component:
        //   - if all members already belong to the SAME nation → keep as-is
        //   - if mixed (some nationless, some from different nations) → the
        //     most-populous existing nation absorbs the rest; nationless are
        //     inducted into it.
        //   - if all nationless AND size >= NATION_FOUND_THRESHOLD → found new
        //     nation.

        // We'll accumulate log messages and apply them after the loop.
        let mut log_msgs: Vec<String> = Vec::new();
        let year = self.year;

        // Map nation id -> index in self.nations (for fast lookup during merge).
        let mut nid_to_idx: std::collections::HashMap<u32, usize> =
            std::collections::HashMap::new();
        for (ni, nation) in self.nations.iter().enumerate() {
            nid_to_idx.insert(nation.id, ni);
        }

        for (_root, members) in &components {
            // ---- 1. Group existing-nation members by culture ------------------
            // Same-culture nations will be merged; different-culture nations
            // co-exist in the same union-find component.
            let mut by_culture: std::collections::BTreeMap<
                u8,
                std::collections::BTreeSet<u32>,
            > = std::collections::BTreeMap::new();
            // nationless_si inherits members' si-ascending order (deterministic).
            let mut nationless_si: Vec<usize> = Vec::new();

            for &si in members {
                let nid = self.nation_of[si];
                if nid < 0 {
                    nationless_si.push(si);
                    continue;
                }
                if let Some(&ni) = nid_to_idx.get(&(nid as u32)) {
                    let culture = self.nations[ni].culture;
                    by_culture.entry(culture).or_default().insert(nid as u32);
                }
            }

            // ---- 2. Per-culture merge -----------------------------------------
            // Within each culture group the most-populous nation absorbs its kin.
            // Tiebreak on smallest nation_id is automatic (BTreeSet ascending order).
            let mut surviving_winners: std::collections::BTreeSet<u32> =
                std::collections::BTreeSet::new();

            for (_culture, nids) in &by_culture {
                // Winner = largest total pop; BTreeSet ascending → smallest id wins ties.
                let winner_nid = *nids
                    .iter()
                    .max_by_key(|&&nid| {
                        nid_to_idx
                            .get(&nid)
                            .map(|&ni| {
                                self.nations[ni]
                                    .territory
                                    .iter()
                                    .map(|&si| self.settlements[si].pop as u64)
                                    .sum::<u64>()
                            })
                            .unwrap_or(0)
                    })
                    .unwrap(); // safe: BTreeSet non-empty by construction

                surviving_winners.insert(winner_nid);

                for &loser_nid in nids.iter().filter(|&&nid| nid != winner_nid) {
                    // Reassign all loser settlements in this component to winner.
                    for &si in members {
                        if self.nation_of[si] == loser_nid as i32 {
                            self.nation_of[si] = winner_nid as i32;
                            if let Some(&wi) = nid_to_idx.get(&winner_nid) {
                                self.nations[wi].territory.push(si);
                            }
                        }
                    }
                    // Log the same-culture absorption.
                    if let Some(&li) = nid_to_idx.get(&loser_nid) {
                        let loser_name = self.nations[li].name.clone();
                        let winner_name = if let Some(&wi) = nid_to_idx.get(&winner_nid) {
                            self.nations[wi].name.clone()
                        } else {
                            "?".to_string()
                        };
                        log_msgs.push(format!(
                            "Y{} {} absorbs its kin {}",
                            year, winner_name, loser_name
                        ));
                    }
                }
            }

            // ---- 3. Nationless assignment ------------------------------------
            if surviving_winners.is_empty() {
                // All nationless — found a new nation if large enough.
                if members.len() >= NATION_FOUND_THRESHOLD {
                    let capital_si = self
                        .highest_pop_settlement(members)
                        .unwrap_or(members[0]);
                    let capital_tile = self.settlements[capital_si].tile;
                    let new_id = self.next_nation_id;
                    self.next_nation_id += 1;
                    let name = self.next_nation_name();
                    let tech: u8 = members
                        .iter()
                        .fold(0u8, |acc, &si| acc | self.settlements[si].tech);
                    // §4 PARKS: inherit lineage traits from founding settlements.
                    let founding_traits: u32 = members
                        .iter()
                        .fold(0u32, |acc, &si| acc | self.settlements[si].traits);
                    let color = nation_color(new_id);
                    // Culture: derived from the capital tile's grid region so
                    // geographically nearby nations share a culture group.
                    let founding_culture: u8 = {
                        let c = capital_tile % COLS;
                        let r = capital_tile / COLS;
                        (((r / 12) * 5) + (c / 12)) as u8
                    };
                    let nation = Nation {
                        id: new_id,
                        name: name.clone(),
                        capital: capital_tile,
                        territory: members.clone(),
                        tech,
                        traits: founding_traits,
                        culture: founding_culture,
                        color,
                        polity: Polity::Balanced,
                        trade_partners: 0,
                        golden: false,
                    };
                    let nation_idx = self.nations.len();
                    nid_to_idx.insert(new_id, nation_idx);
                    self.nations.push(nation);
                    for &si in members {
                        self.nation_of[si] = new_id as i32;
                    }
                    // §8 event-flash: newly-founded nation capital.
                    self.push_flash(capital_tile);
                    log_msgs.push(format!(
                        "Y{} the {} people unite under one banner",
                        year, name
                    ));
                }
            } else if surviving_winners.len() == 1 {
                // Single survivor — induct all nationless into it.
                let only_nid = *surviving_winners.iter().next().unwrap();
                for &si in &nationless_si {
                    self.nation_of[si] = only_nid as i32;
                    if let Some(&ni) = nid_to_idx.get(&only_nid) {
                        self.nations[ni].territory.push(si);
                    }
                }
            } else {
                // Multiple survivors co-exist in the same component (new case:
                // different-culture nations sharing a union-find component).
                // Assign each nationless settlement to the nearest survivor nation
                // by Chebyshev distance.  Tiebreak: smallest nation_id (guaranteed
                // by BTreeSet ascending iteration order).
                for &si in &nationless_si {
                    let tile_si = self.settlements[si].tile;
                    let mut best: Option<(i32, u32)> = None; // (chebyshev_dist, nation_id)
                    for &nid in surviving_winners.iter() {
                        if nid_to_idx.get(&nid).is_some() {
                            let min_d = members
                                .iter()
                                .filter(|&&msi| self.nation_of[msi] == nid as i32)
                                .map(|&msi| {
                                    Continent::tile_dist(
                                        tile_si,
                                        self.settlements[msi].tile,
                                    )
                                })
                                .min()
                                .unwrap_or(i32::MAX);
                            match best {
                                None => best = Some((min_d, nid)),
                                Some((bd, _)) if min_d < bd => best = Some((min_d, nid)),
                                _ => {} // equal dist: keep earlier (smaller) nid from BTreeSet
                            }
                        }
                    }
                    if let Some((_, assigned_nid)) = best {
                        self.nation_of[si] = assigned_nid as i32;
                        if let Some(&ni) = nid_to_idx.get(&assigned_nid) {
                            self.nations[ni].territory.push(si);
                        }
                    }
                }
            }
        }

        // ---- 3. REBUILD territory lists so they reflect reality -------------
        // (The unify/absorb loop may double-count; rebuild from nation_of.)
        for nation in &mut self.nations {
            nation.territory.clear();
        }
        for si in 0..n_sett {
            let nid = self.nation_of[si];
            if nid < 0 {
                continue;
            }
            if let Some(&ni) = nid_to_idx.get(&(nid as u32)) {
                self.nations[ni].territory.push(si);
            }
        }

        // ---- 4. Sync tech union on each nation ------------------------------
        for nation in &mut self.nations {
            nation.tech = nation
                .territory
                .iter()
                .fold(0u8, |acc, &si| acc | self.settlements[si].tech);
        }

        // ---- 5. SPLIT: over-extension or capital-loss -----------------------
        // Collect split work up-front (to avoid borrow issues).
        let mut splits: Vec<(usize /*nation_idx*/, String /*reason*/)> = Vec::new();

        let current_era = self.current_era();
        for (ni, nation) in self.nations.iter().enumerate() {
            let limit = Continent::admin_limit(nation.tech, current_era);
            // Capital is lost only if the capital TILE is empty or now held by a
            // DIFFERENT nation — not merely "some settlement sits there".
            let cap_occ = self.occupied[nation.capital];
            let capital_lost = cap_occ < 0
                || self.nation_of.get(cap_occ as usize).copied().unwrap_or(-1)
                    != nation.id as i32;
            // §4 PARKS — split resistance: a nation that earned DISCIPLINE through
            // sustained warfare AND has reached Medieval technology (3+ techs) can
            // sustain a larger empire without fragmenting.  This is the key link in
            // the DISCIPLINE → split-resistance → Unification chain.
            // Capital-loss splits are NOT bypassed — discipline doesn't prevent
            // administrative collapse when the seat of power falls.
            let nation_era = Era::from_tech_count(nation.tech.count_ones() as u32);
            let immune_to_overextension = (nation.traits & TRAIT_DISCIPLINE != 0)
                && nation_era.index() >= Era::Medieval.index();
            if nation.territory.len() > limit && !immune_to_overextension {
                splits.push((ni, "overextension".to_string()));
            } else if capital_lost {
                splits.push((ni, "capital_loss".to_string()));
            }
        }

        for (ni, reason) in splits {
            // Elect a new capital = highest-pop settlement in territory.
            let territory = self.nations[ni].territory.clone();
            if territory.is_empty() {
                continue;
            }

            if reason == "capital_loss" {
                // Elect a new capital from survivors.
                if let Some(new_cap_si) = self.highest_pop_settlement(&territory) {
                    let new_cap_tile = self.settlements[new_cap_si].tile;
                    let nation_name = self.nations[ni].name.clone();
                    self.nations[ni].capital = new_cap_tile;
                    log_msgs.push(format!(
                        "Y{} the capital of {} falls; the realm splinters",
                        year, nation_name
                    ));
                }
            } else {
                // Over-extension split: carve off the larger half as a new nation.
                // Simple split: first half keeps the old nation; second becomes new.
                if territory.len() < 2 {
                    continue;
                }
                let split_at = territory.len() / 2;
                let remainder = territory[split_at..].to_vec();
                let keep = territory[..split_at].to_vec();

                // New nation from remainder.
                if let Some(new_cap_si) = self.highest_pop_settlement(&remainder) {
                    let new_cap_tile = self.settlements[new_cap_si].tile;
                    let new_id = self.next_nation_id;
                    self.next_nation_id += 1;
                    let new_name = self.next_nation_name();
                    let new_tech: u8 = remainder
                        .iter()
                        .fold(0u8, |acc, &si| acc | self.settlements[si].tech);
                    // §4 PARKS: inherit parent nation's traits + lineage traits on split.
                    let parent_traits = self.nations[ni].traits;
                    let lineage = self.lineage_traits.get(&self.nations[ni].id).copied().unwrap_or(0);
                    let new_traits = parent_traits | lineage
                        | remainder.iter().fold(0u32, |acc, &si| acc | self.settlements[si].traits);
                    // Culture: daughter inherits parent's culture unchanged.
                    let inherited_culture = self.nations[ni].culture;
                    let new_color = nation_color(new_id);
                    let old_name = self.nations[ni].name.clone();

                    // Update nation_of for the departing settlements.
                    for &si in &remainder {
                        self.nation_of[si] = new_id as i32;
                    }
                    // Update the old nation's territory.
                    self.nations[ni].territory = keep;
                    // Rebuild old nation capital if needed.
                    if let Some(new_old_cap) =
                        self.highest_pop_settlement(&self.nations[ni].territory.clone())
                    {
                        let old_cap_tile = self.settlements[new_old_cap].tile;
                        self.nations[ni].capital = old_cap_tile;
                    }

                    let new_nation = Nation {
                        id: new_id,
                        name: new_name.clone(),
                        capital: new_cap_tile,
                        territory: remainder,
                        tech: new_tech,
                        traits: new_traits,
                        culture: inherited_culture,
                        color: new_color,
                        polity: Polity::Balanced,
                        trade_partners: 0,
                        golden: false,
                    };
                    nid_to_idx.insert(new_id, self.nations.len());
                    self.nations.push(new_nation);

                    log_msgs.push(format!(
                        "Y{} {} fractures into two; {} breaks away",
                        year, old_name, new_name
                    ));
                }
            }
        }

        // ---- 6. Remove empty nations ----------------------------------------
        self.nations.retain(|n| !n.territory.is_empty());

        // ---- 7. Emit log messages -------------------------------------------
        for msg in log_msgs {
            self.log_event(msg);
        }

        // ---- 8. Derive polity for each nation from traits + territory size ---
        for nation in &mut self.nations {
            nation.polity = if nation.traits & TRAIT_DISCIPLINE != 0 {
                Polity::Militarist
            } else if nation.traits & (TRAIT_COMMERCE | TRAIT_SEAFARING_TRAIT) != 0 {
                Polity::Merchant
            } else if nation.territory.len() <= 3 {
                Polity::Isolationist
            } else if nation.territory.len() >= 8 {
                Polity::Expansionist
            } else {
                Polity::Balanced
            };
        }
    }

    // ---- §4 PARKS / traits system ------------------------------------------

    /// Returns trait multipliers that affect defense_bonus in WAR.
    /// MASONRY/DISCIPLINE add +0.15 each.
    pub(crate) fn trait_defense_bonus(traits: u32) -> f32 {
        let mut b = 0.0f32;
        if traits & TRAIT_MASONRY != 0 { b += 0.15; }
        if traits & TRAIT_DISCIPLINE != 0 { b += 0.15; }
        b
    }

    // ---- trade system -------------------------------------------------------

    /// Computes trade-partner counts for every nation. Called each nations-cadence
    /// step (right after step_nations so territory lists are fresh).
    ///
    /// A trade link exists between two distinct nations A and B when:
    ///   - at least one settlement of A is within TRADE_DIST of some settlement of B, AND
    ///   - they are NOT currently at war.
    ///
    /// TRADE_DIST is intentionally larger than COHESION_DIST: COHESION_DIST
    /// collapses nearby settlements into one nation, so a separate peaceful
    /// neighbour must be at distance > COHESION_DIST; TRADE_DIST bridges that
    /// gap and allows >= 2 partners (needed for golden ages) to be reachable.
    ///
    /// Complexity: O(N_nations^2 * max_territory^2) but nation/settlement counts
    /// are small (< ~20 nations, < ~250 settlements total in practice) so this
    /// stays fast.  The inner check bails as soon as one qualifying pair is found.
    pub(crate) fn step_trade(&mut self) {
        let n = self.nations.len();
        if n == 0 {
            return;
        }

        // Build a HashSet of warring nation-id pairs for O(1) lookup.
        let at_war: std::collections::HashSet<(u32, u32)> = self
            .wars
            .iter()
            .map(|w| (w.a.min(w.b), w.a.max(w.b)))
            .collect();

        // Collect tile positions per nation index (avoids repeated iteration).
        let tiles_per_nation: Vec<Vec<(i32, i32)>> = self
            .nations
            .iter()
            .map(|nation| {
                nation
                    .territory
                    .iter()
                    .map(|&si| {
                        let t = self.settlements[si].tile;
                        ((t % COLS) as i32, (t / COLS) as i32)
                    })
                    .collect()
            })
            .collect();

        // Count trade partners for each nation.
        let mut partner_counts = vec![0u32; n];
        for a in 0..n {
            for b in (a + 1)..n {
                let id_a = self.nations[a].id;
                let id_b = self.nations[b].id;
                let pair = (id_a.min(id_b), id_a.max(id_b));
                if at_war.contains(&pair) {
                    continue; // at war — no trade
                }
                // Check if any settlement of A is within TRADE_DIST of any settlement of B.
                let mut linked = false;
                'outer: for &(ax, ay) in &tiles_per_nation[a] {
                    for &(bx, by) in &tiles_per_nation[b] {
                        let dist = (ax - bx).abs().max((ay - by).abs());
                        if dist <= TRADE_DIST {
                            linked = true;
                            break 'outer;
                        }
                    }
                }
                if linked {
                    partner_counts[a] += 1;
                    partner_counts[b] += 1;
                }
            }
        }

        // Build war-id set for golden-age check (nation not in any war).
        let at_war_ids: std::collections::HashSet<u32> = self
            .wars
            .iter()
            .flat_map(|w| [w.a, w.b])
            .collect();

        let year = self.year;
        let mut golden_log: Vec<String> = Vec::new();
        // Collect capitals of newly-golden nations before mutably borrowing nations.
        let mut golden_capitals: Vec<usize> = Vec::new();
        for (ni, nation) in self.nations.iter_mut().enumerate() {
            nation.trade_partners = partner_counts[ni];
            let new_golden = partner_counts[ni] >= 2 && !at_war_ids.contains(&nation.id);
            // Rising edge: false -> true, log once.
            if new_golden && !nation.golden {
                golden_log.push(format!("Y{} {} enters a golden age", year, nation.name));
                // §8 event-flash: record capital for flash (pushed after iter ends).
                golden_capitals.push(nation.capital);
            }
            nation.golden = new_golden;
        }
        // Push flashes outside the mutable borrow of nations.
        for cap in golden_capitals {
            self.push_flash(cap);
        }
        for msg in golden_log {
            self.log_event(msg);
        }

        // ---- Plague spreads along trade roads ----------------------------------
        // For each currently-afflicted nation, attempt at most one spread per
        // NATIONS_CADENCE window to a peaceful trade partner.  The probability
        // decays as the plague ages so long-established plagues gradually die out.
        // We collect all spread events before applying them to avoid cascade loops
        // within a single tick.
        if !self.plague_nations.is_empty() {
            // Build war set for quick lookup.
            let at_war_spread: std::collections::HashSet<(u32, u32)> = self
                .wars
                .iter()
                .map(|w| (w.a.min(w.b), w.a.max(w.b)))
                .collect();

            // Gather (afflicted_nation_id, candidate partner ids) using the
            // already-computed tiles_per_nation data.  We rebuild a small
            // trade-link list here (O(N^2) but N is tiny).
            struct SpreadCandidate {
                tgt_nation_id: u32,
                tgt_name: String,
                tgt_tile: usize, // a settlement tile of the target nation
            }
            let mut candidates: Vec<SpreadCandidate> = Vec::new();

            // snapshot of afflicted ids (keys only) to avoid holding borrow on
            // plague_nations while we call roll().
            // SORTED by nation id (ascending) so the roll() sequence is identical
            // across processes for the same seed, regardless of HashMap iteration order.
            let mut afflicted_ids: Vec<(u32, u32)> = self
                .plague_nations
                .iter()
                .map(|(&id, &yr)| (id, yr))
                .collect();
            afflicted_ids.sort_unstable_by_key(|&(id, _)| id);

            let year_now = self.year;

            for (src_id, first_year) in &afflicted_ids {
                // Decaying probability: starts at PLAGUE_SPREAD_BASE, halves every
                // PLAGUE_SPREAD_DECAY_YEARS / 2 years, floored at ~1%.
                let age = year_now.saturating_sub(*first_year);
                let decay = (-(age as f32) / PLAGUE_SPREAD_DECAY_YEARS as f32 * 2.0_f32.ln()).exp();
                let spread_p = (PLAGUE_SPREAD_BASE * decay).max(0.01);

                if self.roll() >= spread_p {
                    continue; // no spread this window for this nation
                }

                // Find the src nation index.
                let src_ni = match self.nations.iter().position(|n| n.id == *src_id) {
                    Some(i) => i,
                    None => continue,
                };

                // Collect candidate partner nations (peaceful, distinct, within
                // COHESION_DIST of at least one src settlement, not already afflicted).
                let src_tiles: Vec<(i32, i32)> = self.nations[src_ni]
                    .territory
                    .iter()
                    .map(|&si| {
                        let t = self.settlements[si].tile;
                        ((t % COLS) as i32, (t / COLS) as i32)
                    })
                    .collect();

                let n_nations = self.nations.len();
                let mut local_cands: Vec<SpreadCandidate> = Vec::new();
                for bi in 0..n_nations {
                    if bi == src_ni {
                        continue;
                    }
                    let tgt_id = self.nations[bi].id;
                    // Skip if already afflicted.
                    if self.plague_nations.contains_key(&tgt_id) {
                        continue;
                    }
                    // Skip if at war.
                    let pair = (src_id.min(&tgt_id), src_id.max(&tgt_id));
                    if at_war_spread.contains(&(*pair.0, *pair.1)) {
                        continue;
                    }
                    // Check proximity (trade link — uses TRADE_DIST, same as step_trade).
                    let mut linked = false;
                    'link: for &(ax, ay) in &src_tiles {
                        for &si in &self.nations[bi].territory {
                            let t = self.settlements[si].tile;
                            let bx = (t % COLS) as i32;
                            let by = (t / COLS) as i32;
                            if (ax - bx).abs().max((ay - by).abs()) <= TRADE_DIST {
                                linked = true;
                                break 'link;
                            }
                        }
                    }
                    if !linked {
                        continue;
                    }
                    // Pick a settlement tile of the target nation (first one).
                    if let Some(&tgt_si) = self.nations[bi].territory.first() {
                        let tgt_tile = self.settlements[tgt_si].tile;
                        local_cands.push(SpreadCandidate {
                            tgt_nation_id: tgt_id,
                            tgt_name: self.nations[bi].name.clone(),
                            tgt_tile,
                        });
                    }
                }

                // Pick at most one target at random.
                if local_cands.is_empty() {
                    continue;
                }
                let pick = (self.roll() * local_cands.len() as f32) as usize;
                let pick = pick.min(local_cands.len() - 1);
                candidates.push(local_cands.remove(pick));
            }

            // Apply spread events (one per afflicted nation, no cascade within tick).
            let mut spread_log: Vec<String> = Vec::new();
            for sc in candidates {
                // Mark the target as newly afflicted.
                let year_now2 = self.year;
                self.plague_nations.entry(sc.tgt_nation_id).or_insert(year_now2);
                let tgt_tile = sc.tgt_tile;
                // Trigger the disaster on a settlement tile of the target nation.
                // We call apply_disaster which will re-insert into plague_nations
                // (harmlessly via entry().or_insert) and handle pop loss.
                self.apply_disaster(DisasterKind::Plague, tgt_tile);
                spread_log.push(format!(
                    "Y{} plague spreads along the trade roads to {}",
                    year_now2, sc.tgt_name
                ));
            }
            for msg in spread_log {
                self.log_event(msg);
            }

            // Prune nations that no longer exist from plague_nations.
            let live_ids: std::collections::HashSet<u32> =
                self.nations.iter().map(|n| n.id).collect();
            // Also expire entries whose affliction has exceeded PLAGUE_SPREAD_DECAY_YEARS.
            let year_now_prune = self.year;
            self.plague_nations.retain(|id, &mut afflicted_year| {
                live_ids.contains(id)
                    && year_now_prune.saturating_sub(afflicted_year) <= PLAGUE_SPREAD_DECAY_YEARS
            });
        }
    }
}
