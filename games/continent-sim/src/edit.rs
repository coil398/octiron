use crate::*;

// ---- §6 edit mode ----------------------------------------------------------

/// Whether the player is observing or actively editing the world.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditMode {
    Observe,
    Edit,
}

/// What the brush currently does in Edit mode.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditTool {
    /// Left-click cycles terrain type; scroll raises/lowers K.
    PaintTerrain,
    /// Trigger the selected disaster kind at the cursor.
    Disaster(DisasterKind),
    /// Grant a "grace" growth nudge to the settlement under the cursor.
    Grace,
    /// Grant divine inspiration to a settlement (temporary growth multiplier, §6).
    Inspire,
    /// Found a new settlement on an unoccupied habitable tile (§6 Found).
    Found,
}

impl EditTool {
    pub(crate) fn label_jp(self) -> &'static str {
        match self {
            EditTool::PaintTerrain                    => "塗装",
            EditTool::Disaster(DisasterKind::Volcano) => "災害:火山噴火",
            EditTool::Disaster(DisasterKind::Quake)   => "災害:地震",
            EditTool::Disaster(DisasterKind::Flood)   => "災害:洪水",
            EditTool::Disaster(DisasterKind::Drought) => "災害:干ばつ",
            EditTool::Disaster(DisasterKind::Plague)  => "災害:疫病",
            EditTool::Disaster(DisasterKind::Tsunami) => "災害:津波",
            EditTool::Grace                           => "恵",
            EditTool::Inspire                         => "鼓舞",
            EditTool::Found                           => "建国",
        }
    }
}

impl Continent {
    /// Paints a new terrain type onto tile `i`, spending faith, then recomputes
    /// K for that tile and its neighbours.
    pub(crate) fn edit_paint_terrain(&mut self, i: usize) {
        if self.faith < FAITH_COST_PAINT {
            return;
        }
        self.faith -= FAITH_COST_PAINT;
        // Cycle to the next terrain type.
        let next_idx = (self.tiles[i].terrain.index() + 1) % TERRAIN_COUNT;
        let new_terrain = ALL_TERRAINS[next_idx];
        self.tiles[i].terrain = new_terrain;
        // Recompute this tile and all neighbours.
        let nbrs = self.neighbours(i);
        self.recompute_tile_k(i);
        for ni in nbrs {
            self.recompute_tile_k(ni);
        }
        // If the tile is no longer habitable (painted into water/mountain/barren)
        // but a settlement sits on it, push it off — mirrors the disaster path so a
        // settlement is never stranded on a 0-K tile (it would otherwise persist,
        // never growing and never migrating).
        if !self.tiles[i].habitable && self.occupied[i] >= 0 {
            self.force_migrate(i);
        }
        let year = self.year;
        self.log_event(format!(
            "Y{} divine will reshapes a tile to {}",
            year,
            new_terrain.tile_name()
        ));
    }

    /// Raises (scroll up) or lowers (scroll down) the K of tile `i` by a fixed
    /// delta fraction, spending faith.
    pub(crate) fn edit_adjust_elevation(&mut self, i: usize, delta_sign: f32) {
        if self.faith < FAITH_COST_ELEVATE {
            return;
        }
        self.faith -= FAITH_COST_ELEVATE;
        let delta = delta_sign * ELEVATION_K_DELTA * K_MAX;
        let old_k = self.tiles[i].k;
        let new_k = (old_k + delta).clamp(0.0, K_MAX * 1.4);
        let was_hab = self.tiles[i].habitable;
        let t = self.tiles[i].terrain;
        let now_hab = !t.is_water() && t != Terrain::Mountain && new_k >= MIN_HABITABLE;
        self.tiles[i].k = new_k;
        self.tiles[i].habitable = now_hab;
        match (was_hab, now_hab) {
            (false, true) => self.habitable_count += 1,
            (true, false) => self.habitable_count = self.habitable_count.saturating_sub(1),
            _ => {}
        }
        // Recompute neighbours as well so their habitability stays consistent.
        let nbrs = self.neighbours(i);
        for ni in nbrs {
            self.recompute_tile_k(ni);
        }
    }

    /// Grants a "grace" nudge: if there is a settlement on tile `i`, boost its
    /// population by 20 % (up to K), spending faith.
    pub(crate) fn edit_grace(&mut self, i: usize) {
        if self.faith < FAITH_COST_GRACE {
            return;
        }
        let si = self.occupied[i];
        if si < 0 {
            return;
        }
        self.faith -= FAITH_COST_GRACE;
        let si = si as usize;
        let k = self.tiles[i].k;
        let pop = self.settlements[si].pop;
        self.settlements[si].pop = (pop * 1.20).min(k);
        let year = self.year;
        self.log_event(format!("Y{} divine grace blesses a settlement", year));
    }

    /// Grants divine inspiration to the settlement on tile `i` (§6 Inspire).
    /// Costs FAITH_COST_INSPIRE faith.  Sets `inspired_years` to 10 so the
    /// settlement receives a ×1.15 growth multiplier for the next 10 ticks.
    pub(crate) fn edit_inspire(&mut self, i: usize) {
        if self.faith < FAITH_COST_INSPIRE {
            return;
        }
        let si = self.occupied[i];
        if si < 0 {
            return;
        }
        self.faith -= FAITH_COST_INSPIRE;
        let si = si as usize;
        self.settlements[si].inspired_years = 10;
        let year = self.year;
        self.log_event(format!("Y{} divine inspiration stirs a people", year));
    }

    /// Plants a new people on an unoccupied habitable tile (§6 Found).
    /// Costs FAITH_COST_FOUND faith.  Does nothing (no faith spent) if the tile
    /// is occupied, uninhabitable, or if faith is insufficient.
    pub(crate) fn edit_found(&mut self, i: usize) {
        // Guard: must have enough faith.
        if self.faith < FAITH_COST_FOUND {
            return;
        }
        // Guard: tile must be habitable and unoccupied.
        if !self.tiles[i].habitable || self.occupied[i] >= 0 {
            return;
        }
        self.faith -= FAITH_COST_FOUND;
        let new_si = self.settlements.len() as i32;
        self.settlements.push(Settlement {
            tile: i,
            pop: 20.0,
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
        self.occupied[i] = new_si;
        // Extend nation_of so the new settlement has no nation yet (-1).
        self.nation_of.push(-1);
        let year = self.year;
        self.log_event(format!("Y{} divine will plants a new people", year));
    }

    /// Regen faith by FAITH_REGEN per simulated year.
    /// resourcefx (b): each settlement sitting on a Gold tile adds a small bonus
    /// (0.05 faith/year) so Gold-rich nations gradually top up the faith pool faster.
    pub(crate) fn regen_faith(&mut self) {
        self.faith_accum += FAITH_REGEN;

        // resourcefx (b): Gold-tile bonus — count settlements on Gold tiles.
        let gold_bonus: f32 = self.settlements.iter().filter(|s| {
            s.tile < self.resources.len() && self.resources[s.tile] == Resource::Gold
        }).count() as f32 * 0.05;
        self.faith_accum += gold_bonus;

        let whole = self.faith_accum.floor();
        self.faith_accum -= whole;
        self.faith = (self.faith + whole).min(FAITH_MAX);
    }

    /// Cycles the edit tool forward through: PaintTerrain → Disaster(Volcano) →
    /// Disaster(Quake) → Disaster(Flood) → Disaster(Drought) → Disaster(Plague)
    /// → Grace → PaintTerrain.
    pub(crate) fn cycle_tool(&mut self) {
        self.edit_tool = match self.edit_tool {
            EditTool::PaintTerrain => EditTool::Disaster(DisasterKind::Volcano),
            EditTool::Disaster(DisasterKind::Volcano) => EditTool::Disaster(DisasterKind::Quake),
            EditTool::Disaster(DisasterKind::Quake) => EditTool::Disaster(DisasterKind::Flood),
            EditTool::Disaster(DisasterKind::Flood) => EditTool::Disaster(DisasterKind::Drought),
            EditTool::Disaster(DisasterKind::Drought) => EditTool::Disaster(DisasterKind::Plague),
            EditTool::Disaster(DisasterKind::Plague) => EditTool::Disaster(DisasterKind::Tsunami),
            EditTool::Disaster(DisasterKind::Tsunami) => EditTool::Grace,
            EditTool::Grace => EditTool::Inspire,
            EditTool::Inspire => EditTool::Found,
            EditTool::Found => EditTool::PaintTerrain,
        };
    }

    /// §6: draws the god intervention HUD strip just below the map area,
    /// overlaid on the status bar when in EDIT mode.
    pub(crate) fn draw_intervention_hud(&self, painter: &mut Painter) {
        // HUD background bar.
        let hud_y = MAP_Y + MAP_H - 20.0;
        let hud_h = 20.0;

        if self.edit_mode == EditMode::Observe {
            // Subtle label only.
            self.pt(painter, MAP_X, hud_y + 4.0, 0.36, [0.40, 0.45, 0.55, 1.0],
                "[M] 観察モード  -- M で編集モードへ");
            return;
        }

        // Edit mode active.
        painter.rect(MAP_X, hud_y, MAP_W, hud_h, [0.08, 0.10, 0.20, 0.88]);

        // Faith bar (left side).
        let bar_w = 120.0;
        let faith_frac = (self.faith / FAITH_MAX).clamp(0.0, 1.0);
        // Background.
        painter.rect(MAP_X + 4.0, hud_y + 4.0, bar_w, 12.0, [0.20, 0.20, 0.30, 1.0]);
        // Filled.
        let bar_color = if self.faith < 10.0 {
            [0.85, 0.25, 0.20, 1.0]
        } else {
            [0.50, 0.80, 1.00, 1.0]
        };
        painter.rect(MAP_X + 4.0, hud_y + 4.0, bar_w * faith_frac, 12.0, bar_color);
        self.pt(painter, MAP_X + 4.0, hud_y + 4.0, 0.34, [1.0, 1.0, 1.0, 1.0],
            &format!("信仰 {:.0}/{:.0}", self.faith, FAITH_MAX));

        // Tool label (right of faith bar).
        let tool_label = self.edit_tool.label_jp();
        self.pt(painter, MAP_X + bar_w + 12.0, hud_y + 4.0, 0.36, [1.0, 0.90, 0.40, 1.0],
            &format!("ツール: {}  [T] 次へ", tool_label));

        // Mode label (far right).
        self.pt(painter, MAP_X + bar_w + 240.0, hud_y + 4.0, 0.36, [0.55, 1.0, 0.55, 1.0],
            "編集モード  [M] 観察");
    }
}
