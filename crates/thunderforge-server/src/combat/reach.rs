//! Reach, range and line of sight, measured from footprint to footprint (spec
//! 046 US4, FR-032–FR-035, decision 3, research R11).
//!
//! One measurement, called by both `make_attack` and `preview_attack`, so the
//! warning a player reads before rolling is exactly what the table is told
//! afterwards.
//!
//! **Flags, never refusals** (clarification 1, contract C3). Nothing here can
//! stop an attack; it describes one. The turn check (C1) is the only rule that
//! refuses a player, and callers make it before they ever get here.
//!
//! # What is measured
//!
//! - **Distance**: [`GridSpec::footprint_distance`] between the squares each
//!   creature fills (sizes from `combat::size`), in cells, times the system's
//!   `vision.unitsPerCell` — so 5e reads feet. Adjacent is one cell: five feet.
//! - **Line of sight**: [`footprint_line_of_sight`] against the scene's walls,
//!   skipped for an ability that does not need it. Walls only — light is what a
//!   viewer is told about (`combat::redaction`), not what a sword is stopped by.
//!
//! # Why redaction does not use this
//!
//! A viewer's "Unknown" is judged from token **centres** with `visibility_of`
//! (range, cone, walls, light), because that is how the engine decides which
//! tokens to draw (spec 045). An attack's line of sight is judged from
//! **footprints**, because a rule about a Large creature has to be about the
//! squares it fills (FR-035). The two can disagree about a big creature half
//! round a corner: it may be attacked (and attack) with line of sight while a
//! player's board — and therefore their log — still does not show it. Moving
//! redaction to footprints would name a creature in the log that the same
//! player's board does not draw; the log follows the board. Research R11 and
//! contracts/fight.md §3 record this.

use std::collections::HashMap;

use diesel::PgConnection;
use diesel::prelude::*;
use thunderforge_canvas_core::Vec2;
use thunderforge_canvas_core::grid::{Footprint, GridSpec};
use thunderforge_canvas_core::measure::GridUnits;
use thunderforge_canvas_core::wall::WallSet;
use uuid::Uuid;

// The tests read the flags by name (`use super::*`).
#[cfg(test)]
use crate::combat::records::{
    FLAG_BEYOND_RANGE, FLAG_LONG_RANGE, FLAG_NO_LINE_OF_SIGHT, FLAG_NO_REACH_DECLARED,
    FLAG_OUT_OF_REACH,
};
use crate::schema::{scenes, tokens, walls, worlds};
use crate::vision_profiles::vision_declaration_for_system;

/// What reaches what — an ability's reach and range, the flags a distance
/// earns, a scene's grid, and the measurement itself — is the shared rules'
/// (`thunderforge_combat::reach`, ADR-113). This module loads a scene for it.
pub use thunderforge_combat::reach::{Measured, Reach, flags_for, scene_grid};

/// Everything one scene is measured by, loaded once for an attack (or a
/// multiattack's several).
pub struct SceneMeasure {
    grid: GridSpec,
    units: GridUnits,
    walls: WallSet,
    placed: HashMap<Uuid, (Vec2, Footprint)>,
}

impl SceneMeasure {
    /// The scene's grid, walls and units, and where and how big the named
    /// tokens are. Walls are loaded only when something needs line of sight.
    pub fn load(
        conn: &mut PgConnection,
        systems_dir: &str,
        scene_id: Uuid,
        token_ids: &[Uuid],
        needs_walls: bool,
    ) -> QueryResult<SceneMeasure> {
        let (grid_type, grid_size, width, height, system_id) = scenes::table
            .inner_join(worlds::table.on(worlds::id.eq(scenes::world_id)))
            .filter(scenes::scene_id.eq(scene_id))
            .select((
                scenes::grid_type,
                scenes::grid_size,
                scenes::width,
                scenes::height,
                worlds::game_system_id,
            ))
            .first::<(String, i32, i32, i32, Option<String>)>(conn)?;
        let units = system_id
            .as_deref()
            .map(|id| vision_declaration_for_system(systems_dir, id).grid_units())
            .unwrap_or_default();

        let sizes = crate::combat::size::footprints_of(conn, systems_dir, scene_id, token_ids)?;
        let placed = tokens::table
            .filter(tokens::scene_id.eq(scene_id))
            .filter(tokens::token_id.eq_any(token_ids))
            .select((tokens::token_id, tokens::x, tokens::y))
            .load::<(Uuid, f64, f64)>(conn)?
            .into_iter()
            .map(|(id, x, y)| {
                let footprint = sizes.get(&id).copied().unwrap_or(1.0);
                (
                    id,
                    (Vec2::new(x as f32, y as f32), Footprint::new(footprint)),
                )
            })
            .collect();

        let walls = if needs_walls {
            let rows = walls::table
                .filter(walls::scene_id.eq(scene_id))
                .select(crate::models::Wall::as_select())
                .load::<crate::models::Wall>(conn)?;
            crate::movement::wall_set_from_rows(&rows)
        } else {
            WallSet::default()
        };

        Ok(SceneMeasure {
            grid: scene_grid(&grid_type, grid_size, width, height),
            units,
            walls,
            placed,
        })
    }

    /// The system's unit, for saying a distance ("ft").
    pub fn unit_label(&self) -> &str {
        &self.units.label
    }

    /// Distance and flags for one attack. With no target, or a party that is
    /// not on this scene, there is nothing to measure.
    pub fn measure(&self, attacker: Uuid, target: Option<Uuid>, reach: &Reach) -> Measured {
        let Some(target) = target else {
            return Measured::default();
        };
        let (Some(&(from, from_size)), Some(&(to, to_size))) =
            (self.placed.get(&attacker), self.placed.get(&target))
        else {
            return Measured::default();
        };
        thunderforge_combat::reach::measure(
            &self.grid,
            &self.units,
            &self.walls,
            (from, from_size),
            (to, to_size),
            reach,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn melee(reach: f64) -> Reach {
        Reach {
            reach: Some(reach),
            needs_line_of_sight: true,
            ..Default::default()
        }
    }

    fn ranged(normal: f64, long: Option<f64>) -> Reach {
        Reach {
            range_normal: Some(normal),
            range_long: long,
            needs_line_of_sight: true,
            ..Default::default()
        }
    }

    #[test]
    fn within_reach_is_unflagged_and_beyond_it_is_out_of_reach() {
        assert!(flags_for(5.0, &melee(5.0), true).is_empty());
        assert_eq!(flags_for(10.0, &melee(5.0), true), [FLAG_OUT_OF_REACH]);
        assert_eq!(flags_for(20.0, &melee(5.0), true), [FLAG_OUT_OF_REACH]);
    }

    #[test]
    fn a_ranged_attack_is_long_beyond_normal_and_beyond_range_past_long() {
        let shortbow = ranged(80.0, Some(320.0));
        assert!(flags_for(80.0, &shortbow, true).is_empty());
        assert_eq!(flags_for(85.0, &shortbow, true), [FLAG_LONG_RANGE]);
        assert_eq!(flags_for(320.0, &shortbow, true), [FLAG_LONG_RANGE]);
        assert_eq!(flags_for(325.0, &shortbow, true), [FLAG_BEYOND_RANGE]);
        // No long range: anything past normal is beyond range.
        assert_eq!(
            flags_for(35.0, &ranged(30.0, None), true),
            [FLAG_BEYOND_RANGE]
        );
    }

    #[test]
    fn a_thrown_weapon_is_measured_by_its_range_once_past_its_reach() {
        let dagger = Reach {
            reach: Some(5.0),
            range_normal: Some(20.0),
            range_long: Some(60.0),
            needs_line_of_sight: true,
        };
        assert!(flags_for(5.0, &dagger, true).is_empty());
        assert!(flags_for(20.0, &dagger, true).is_empty());
        assert_eq!(flags_for(30.0, &dagger, true), [FLAG_LONG_RANGE]);
        assert_eq!(flags_for(65.0, &dagger, true), [FLAG_BEYOND_RANGE]);
    }

    #[test]
    fn nothing_declared_says_so() {
        let bare = Reach {
            needs_line_of_sight: true,
            ..Default::default()
        };
        assert_eq!(flags_for(5.0, &bare, true), [FLAG_NO_REACH_DECLARED]);
    }

    #[test]
    fn no_line_of_sight_is_flagged_only_when_it_is_needed() {
        assert_eq!(flags_for(5.0, &melee(5.0), false), [FLAG_NO_LINE_OF_SIGHT]);
        assert_eq!(
            flags_for(20.0, &melee(5.0), false),
            [FLAG_OUT_OF_REACH, FLAG_NO_LINE_OF_SIGHT]
        );
        let teleport = Reach {
            needs_line_of_sight: false,
            ..melee(5.0)
        };
        assert!(flags_for(5.0, &teleport, false).is_empty());
    }

    #[test]
    fn a_scene_grid_is_anchored_to_its_map_as_the_engine_anchors_it() {
        let grid = scene_grid("square", 64, 640, 480);
        assert_eq!(grid.origin, Vec2::new(-320.0, -240.0));
        assert_eq!(grid.size, 64.0);
        let no_size = scene_grid("square", 64, 0, 0);
        assert_eq!(no_size.origin, Vec2::ZERO);
        assert!(scene_grid("hex", 64, 640, 480).kind.is_hex());
    }
}
