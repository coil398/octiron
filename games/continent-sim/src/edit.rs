use crate::*;

// ---- §6 edit mode ----------------------------------------------------------

/// Whether the player is observing or actively editing the world.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditMode {
    Observe,
    Edit,
}

/// What a click does during play, before the hidden edit mode.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GodTool {
    Inspect,
    Raise,
    Grace,
    Found,
}

impl GodTool {
    pub(crate) fn label_jp(self) -> &'static str {
        match self {
            GodTool::Inspect => "調べる",
            GodTool::Raise => "上げる",
            GodTool::Grace => "恵み",
            GodTool::Found => "建国",
        }
    }
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
            "Y{} 神意が大地を{}にする",
            year,
            new_terrain.tile_name_jp()
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
        self.log_event(format!("Y{} 神の恵みが集落を祝福する", year));
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
        self.log_event(format!("Y{} 神の鼓舞が民を奮い立たせる", year));
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
        self.log_event(format!("Y{} 神意が新たな民を植える", year));
    }

    /// Shapes the continent from the start: sea becomes plains, a mountain
    /// becomes hills, and other land gains carrying capacity.
    pub(crate) fn god_raise(&mut self, i: usize) {
        if i >= self.tiles.len() || self.faith < FAITH_COST_PAINT {
            return;
        }
        let year = self.year;
        let t = self.tiles[i].terrain;
        if t.is_water() {
            self.faith -= FAITH_COST_PAINT;
            self.tiles[i].terrain = Terrain::Plains;
            self.tiles[i].elevation = self.tiles[i].elevation.max(0.48);
            let nbrs = self.neighbours(i);
            self.recompute_tile_k(i);
            for ni in nbrs {
                self.recompute_tile_k(ni);
            }
            self.log_event(format!("Y{} 神は海を陸に上げた", year));
        } else if t == Terrain::Mountain {
            self.faith -= FAITH_COST_PAINT;
            self.tiles[i].terrain = Terrain::Hills;
            let nbrs = self.neighbours(i);
            self.recompute_tile_k(i);
            for ni in nbrs {
                self.recompute_tile_k(ni);
            }
            self.log_event(format!("Y{} 神は山を丘陵にした", year));
        } else {
            self.faith -= FAITH_COST_PAINT;
            let delta = ELEVATION_K_DELTA * K_MAX;
            let old_k = self.tiles[i].k;
            let new_k = (old_k + delta).clamp(0.0, K_MAX * 1.4);
            let was_hab = self.tiles[i].habitable;
            let now_hab = !t.is_water() && t != Terrain::Mountain && new_k >= MIN_HABITABLE;
            self.tiles[i].k = new_k;
            self.tiles[i].habitable = now_hab;
            match (was_hab, now_hab) {
                (false, true) => self.habitable_count += 1,
                (true, false) => self.habitable_count = self.habitable_count.saturating_sub(1),
                _ => {}
            }
            self.log_event(format!("Y{} 神は大地の実りを増やした", year));
        }
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

    /// Edit-mode card along the bottom of the map. Observe mode draws nothing.
    pub(crate) fn draw_intervention_hud(&self, painter: &mut Painter) {
        if self.edit_mode == EditMode::Observe {
            return;
        }
        let w = MAP_W - 32.0;
        let h = 44.0;
        let x = MAP_X + 16.0;
        let y = MAP_Y + MAP_H - h - 14.0;
        painter.rect(x + 4.0, y + 5.0, w, h, [0.0, 0.0, 0.0, 0.28]);
        ui_card(painter, x, y, w, h, UI_PANEL, UI_LINE, 1.0);
        self.tx(painter, x + 16.0, y + 14.0, 13.0, UI_DIM, "信仰");
        let frac = (self.faith / FAITH_MAX).clamp(0.0, 1.0);
        let bar_x = x + 56.0;
        painter.rect(bar_x, y + 19.0, 140.0, 6.0, UI_TRACK);
        let bar_color = if self.faith < 10.0 { UI_RED } else { UI_ACCENT };
        painter.rect(bar_x, y + 19.0, 140.0 * frac, 6.0, bar_color);
        self.tx(painter, bar_x + 148.0, y + 14.0, 13.0, UI_INK, &format!("{:.0}", self.faith));
        self.tx(painter, x + 250.0, y + 14.0, 14.0, UI_GOLD, self.edit_tool.label_jp());
        let back = "M で観察に戻る";
        let bw = self.tx_w(painter, 13.0, back);
        self.tx(painter, x + w - 16.0 - bw, y + 14.0, 13.0, UI_DIM, back);
    }
}
