//! What a viewer may know of an attack's parties (spec 046 FR-002a, contract §3,
//! research R8).
//!
//! Every seat is told an attack happened. Not every seat is told who made it,
//! or at what: a player who cannot see the ogre that swung at them, or from
//! whom the Game Master hid its name, reads "Unknown" — and the ogre's token
//! id, name and ability are not in the answer at all, rather than in it and
//! left undrawn. That is stricter than spec 045's tokens, which are sent and
//! not drawn, and it is deliberately so (research R8's consistency note).
//!
//! # Decided when the answer is built
//!
//! Not when the attack was made. The same attack reads differently to two
//! players, and differently to one player after they walk round the corner.
//! Nothing a redaction hides is ever stored anywhere a client can reach — the
//! event carries an id (contract §4) — so there is no copy to leak.
//!
//! # "Can see"
//!
//! A token is seen when any token the viewer controls (research R7) perceives
//! it under `thunderforge_canvas_core::vision::visibility_of` — the function
//! the engine hides tokens with — against the scene's walls, its lights (the
//! Game Master's and every carried one), its ambient light, and each eye's
//! own darkvision as its game system declares it (`vision_profiles`). A token
//! the viewer controls is always known to them: the target of an attack is
//! told what hit it, even when the attacker is "Unknown". A viewer with no
//! token in the scene sees nobody. A Game Master sees everyone.

use std::collections::{HashMap, HashSet};

use diesel::PgConnection;
use diesel::prelude::*;
use thunderforge_canvas_core::Vec2;
use thunderforge_canvas_core::lighting::LightSource;
use thunderforge_canvas_core::vision::{
    AmbientLight, Illumination, ResolvedLight, Rgb, Visibility, VisionProfile, visibility_of,
};
use thunderforge_canvas_core::wall::WallSet;
use uuid::Uuid;

use crate::declared_values::ActorSlots;
use crate::schema::{light_sources, scenes, tokens, walls, world_actor_system_data, worlds};
use crate::vision_profiles::{cells_to_world, resolve, vision_declaration_for_system};

/// What a viewer is told a redacted party is called.
pub const UNKNOWN: &str = "Unknown";

/// One side of an attack, as a viewer may know it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Party {
    /// `None` when redacted, or when the token is gone.
    pub token_id: Option<Uuid>,
    pub label: String,
    pub redacted: bool,
}

impl Party {
    fn unknown() -> Self {
        Party {
            token_id: None,
            label: UNKNOWN.to_string(),
            redacted: true,
        }
    }
}

struct Placed {
    at: Vec2,
    name_visible: bool,
}

/// A scene as one viewer sees it, loaded once and asked about many tokens.
pub struct SceneSight {
    runs_the_world: bool,
    controlled: HashSet<Uuid>,
    placed: HashMap<Uuid, Placed>,
    eyes: Vec<(Vec2, VisionProfile)>,
    lights: Vec<ResolvedLight>,
    walls: WallSet,
    ambient: AmbientLight,
}

/// The scene's ambient light, as `updateSceneAmbientLight` stores it.
fn ambient_from(stored: &str) -> AmbientLight {
    let level = match stored.trim().to_ascii_lowercase().as_str() {
        "dark" => Illumination::Dark,
        "dim" => Illumination::Dim,
        _ => Illumination::Bright,
    };
    AmbientLight { level, color: None }
}

fn resolved(light: &LightSource, at: Vec2) -> ResolvedLight {
    ResolvedLight {
        position: at,
        bright_radius: light.bright(),
        dim_radius: light.radius,
        color: light
            .color
            .as_deref()
            .and_then(Rgb::parse_hex)
            .unwrap_or(Rgb::WHITE),
        intensity: light.intensity,
        casts_shadows: light.casts_shadows,
    }
}

impl SceneSight {
    /// Everything `user_id` sees `scene_id` by. A Game Master's is empty and
    /// sees everything, so nothing is loaded for one.
    pub fn for_viewer(
        conn: &mut PgConnection,
        systems_dir: &str,
        user_id: Uuid,
        is_admin: bool,
        scene_id: Uuid,
    ) -> QueryResult<SceneSight> {
        let (world_id, grid_size, ambient_light) = scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .select((scenes::world_id, scenes::grid_size, scenes::ambient_light))
            .first::<(Uuid, i32, String)>(conn)?;

        let runs_the_world =
            crate::auth::world_membership::actor_in_world(conn, user_id, is_admin, world_id)
                .runs_the_world();
        let mut sight = SceneSight {
            runs_the_world,
            controlled: HashSet::new(),
            placed: HashMap::new(),
            eyes: Vec::new(),
            lights: Vec::new(),
            walls: WallSet::default(),
            ambient: ambient_from(&ambient_light),
        };
        if runs_the_world {
            return Ok(sight);
        }

        sight.controlled =
            crate::combat::controllers::controlled_tokens_in_scene(conn, user_id, scene_id)?
                .into_iter()
                .collect();

        let rows = tokens::table
            .filter(tokens::scene_id.eq(scene_id))
            .select((
                tokens::token_id,
                tokens::x,
                tokens::y,
                tokens::actor_id,
                tokens::name_visible_to_players,
            ))
            .load::<(Uuid, f64, f64, Option<Uuid>, bool)>(conn)?;
        for (token_id, x, y, _, name_visible) in &rows {
            sight.placed.insert(
                *token_id,
                Placed {
                    at: Vec2::new(*x as f32, *y as f32),
                    name_visible: *name_visible,
                },
            );
        }
        // A viewer with no token here sees nobody: nothing else is needed.
        if sight.controlled.is_empty() {
            return Ok(sight);
        }

        let wall_rows = walls::table
            .filter(walls::scene_id.eq(scene_id))
            .select(crate::models::Wall::as_select())
            .load::<crate::models::Wall>(conn)?;
        sight.walls = crate::movement::wall_set_from_rows(&wall_rows);

        // The system's sight: every eye's darkvision and every carried light,
        // from each creature's own sheet, as `tokenVision` resolves them.
        let system_id = worlds::table
            .filter(worlds::id.eq(world_id))
            .select(worlds::game_system_id)
            .first::<Option<String>>(conn)?;
        let declaration = system_id
            .as_deref()
            .map(|id| vision_declaration_for_system(systems_dir, id))
            .unwrap_or_default();
        let declared = declaration != Default::default();
        let slots: HashMap<Uuid, ActorSlots> = if declared {
            let actor_ids: Vec<Uuid> = rows.iter().filter_map(|row| row.3).collect();
            type SlotRow = (
                Uuid,
                Option<serde_json::Value>,
                Option<serde_json::Value>,
                Option<serde_json::Value>,
                Option<serde_json::Value>,
            );
            world_actor_system_data::table
                .filter(world_actor_system_data::actor_id.eq_any(&actor_ids))
                .select((
                    world_actor_system_data::actor_id,
                    world_actor_system_data::ability_data,
                    world_actor_system_data::resource_data,
                    world_actor_system_data::proficiency_data,
                    world_actor_system_data::trait_data,
                ))
                .load::<SlotRow>(conn)?
                .into_iter()
                .map(
                    |(id, ability_data, resource_data, proficiency_data, trait_data)| {
                        (
                            id,
                            ActorSlots {
                                ability_data,
                                resource_data,
                                proficiency_data,
                                trait_data,
                            },
                        )
                    },
                )
                .collect()
        } else {
            HashMap::new()
        };

        for (token_id, x, y, actor_id, _) in &rows {
            let at = Vec2::new(*x as f32, *y as f32);
            let seen_by = actor_id
                .and_then(|id| slots.get(&id))
                .map(|slot| resolve(slot, &declaration));
            if let Some(vision) = &seen_by {
                let bright = cells_to_world(vision.carried_bright, grid_size);
                let dim = cells_to_world(vision.carried_dim, grid_size);
                if let Some(light) = LightSource::carried(&token_id.to_string(), bright, dim) {
                    sight.lights.push(resolved(&light, at));
                }
            }
            if sight.controlled.contains(token_id) {
                let darkvision = seen_by
                    .map(|vision| cells_to_world(vision.darkvision, grid_size))
                    .unwrap_or(0.0);
                sight
                    .eyes
                    .push((at, VisionProfile::with_darkvision(darkvision)));
            }
        }

        let light_rows = light_sources::table
            .filter(light_sources::scene_id.eq(scene_id))
            .select((
                light_sources::light_id,
                light_sources::x,
                light_sources::y,
                light_sources::radius,
                light_sources::intensity,
                light_sources::color,
                light_sources::attached_token_id,
                light_sources::casts_shadows,
            ))
            .load::<(Uuid, f64, f64, f64, f64, Option<String>, Option<Uuid>, bool)>(conn)?;
        for (id, x, y, radius, intensity, color, attached, casts_shadows) in light_rows {
            let at = match attached {
                // An attached light is where its token is, and a light
                // attached to a token that is not here lights nothing.
                Some(token) => match sight.placed.get(&token) {
                    Some(placed) => placed.at,
                    None => continue,
                },
                None => Vec2::new(x as f32, y as f32),
            };
            let light = LightSource {
                id: id.to_string(),
                x: x as f32,
                y: y as f32,
                radius: radius as f32,
                intensity: intensity as f32,
                color,
                attached_token_id: attached.map(|t| t.to_string()),
                casts_shadows,
                bright_radius: None,
            };
            sight.lights.push(resolved(&light, at));
        }

        Ok(sight)
    }

    pub fn runs_the_world(&self) -> bool {
        self.runs_the_world
    }

    /// Whether the viewer controls this token.
    pub fn controls(&self, token_id: Uuid) -> bool {
        self.controlled.contains(&token_id)
    }

    /// Whether any of the viewer's tokens perceives this one.
    fn sees(&self, token_id: Uuid) -> bool {
        let Some(target) = self.placed.get(&token_id) else {
            return false;
        };
        self.eyes.iter().any(|(eye, vision)| {
            // A token never hides from its own square, as in the engine.
            eye.distance(target.at) <= f32::EPSILON
                || visibility_of(
                    *eye,
                    vision,
                    target.at,
                    &self.lights,
                    &self.walls,
                    self.ambient,
                ) != Visibility::Hidden
        })
    }

    /// Whether the viewer may be told which token this is (contract §3).
    pub fn may_know(&self, token_id: Option<Uuid>) -> bool {
        if self.runs_the_world {
            return true;
        }
        let Some(token_id) = token_id else {
            // A deleted token: nothing left to judge it by, so nothing told.
            return false;
        };
        if self.controlled.contains(&token_id) {
            return true;
        }
        let name_visible = self
            .placed
            .get(&token_id)
            .is_some_and(|placed| placed.name_visible);
        name_visible && self.sees(token_id)
    }

    /// One party of an attack, as this viewer may know it.
    pub fn party(&self, token_id: Option<Uuid>, label: &str) -> Party {
        if self.may_know(token_id) {
            Party {
                token_id,
                label: label.to_string(),
                redacted: false,
            }
        } else {
            Party::unknown()
        }
    }
}

/// [`SceneSight::party`] for a single token, loading the scene for it.
pub fn party_for_viewer(
    conn: &mut PgConnection,
    systems_dir: &str,
    viewer: Uuid,
    is_admin: bool,
    token_id: Option<Uuid>,
    label: &str,
    scene_id: Uuid,
) -> QueryResult<Party> {
    let sight = SceneSight::for_viewer(conn, systems_dir, viewer, is_admin, scene_id)?;
    Ok(sight.party(token_id, label))
}

#[cfg(test)]
#[path = "redaction_tests.rs"]
mod tests;
