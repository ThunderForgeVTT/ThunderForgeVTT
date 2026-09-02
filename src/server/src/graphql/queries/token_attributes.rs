//! `tokenAttributes(sceneId)` — each token's attribute scores, as its own
//! system names them.
//!
//! Deliberately a sibling of `tokenStatus` rather than a field on it. The two
//! answer different questions: status is *what this viewer may know*, and
//! carries a per-viewer coarsening model. Attributes are simply what the
//! character sheet says. Folding them into the status resolver would imply a
//! disclosure model that does not exist for them and would have to be
//! invented on the spot — and the first person to add one would find it
//! half-applied.
//!
//! The resolution rules live in `thunderforge_canvas_core::attributes`, where
//! tests execute; the manifest reading lives in `crate::attributes`. This
//! module loads what they need and shapes the answer.

use async_graphql::{Context, Error, Object, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use thunderforge_canvas_core::attributes::{AttributeDeclaration, attributes_from};
use thunderforge_canvas_core::movement_budget::{MovementDeclaration, speeds_from};

use thunderforge_canvas_core::system_rules::{DeclaredValueKind, Origin};

use crate::attributes::{attribute_declarations_for_system, movement_declarations_for_system};
use crate::declared_values::{ActorSlots, declared_values_for_actor};
use crate::graphql::{app_state, authenticated_user};

/// A declared value as text.
///
/// One rendering, here, rather than each surface choosing its own — two
/// clients formatting the same number differently is the sort of difference
/// nobody notices until a Game Master and a player are reading the same sheet
/// and disagreeing about it.
fn render(value: &DeclaredValueKind) -> String {
    match value {
        DeclaredValueKind::Integer(v) => v.to_string(),
        DeclaredValueKind::Number(v) => v.to_string(),
        DeclaredValueKind::Text(v) => v.clone(),
        DeclaredValueKind::Boolean(v) => v.to_string(),
        DeclaredValueKind::List(items) => items.join(", "),
        DeclaredValueKind::Fraction { current, max } => match max {
            Some(max) => format!("{current} / {max}"),
            None => current.to_string(),
        },
    }
}

/// A pool's two numbers, sent as numbers.
///
/// The string above is for reading; this is for drawing. A bar is a
/// proportion and a proportion needs both halves together — sending only the
/// rendered text forced the one consumer that draws bars to parse it back
/// apart, which is branching on what a value means and is exactly what the
/// declared-value contract exists to prevent (spec 032 T019a).
///
/// `max` absent means a counter, not an empty pool: Blades in the Dark's coin
/// counts up with nothing to be a proportion of.
fn fraction_of(value: &DeclaredValueKind) -> Option<GraphQLValueFraction> {
    match value {
        DeclaredValueKind::Fraction { current, max } => Some(GraphQLValueFraction {
            current: *current,
            max: *max,
        }),
        _ => None,
    }
}

/// One attribute score.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLAttribute {
    /// The system's own identifier — `might`, `strength`, `prowess`.
    pub id: String,
    pub label: String,
    /// Short form where the system offers one; systems differ.
    pub abbreviation: Option<String>,
    pub value: i32,
}

/// One movement speed, in the scene's spoken units.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLSpeed {
    /// The system's own identifier — `walk`, `fly`, `stride`.
    pub id: String,
    /// Systems disagree even where ids match: Pathfinder calls its ground
    /// speed simply "Speed".
    pub label: String,
    pub value: f64,
}

/// Whether a value was stored or computed.
///
/// Sent because a surface has to know which numbers a player may edit. A 5e
/// Strength score is typed in; its modifier is not, and a text box over a
/// computed value invites the two to disagree.
#[derive(async_graphql::Enum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum GraphQLValueOrigin {
    Stored,
    Derived,
}

/// A pool's current value and, where the system gives one, its maximum.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLValueFraction {
    pub current: i32,
    /// Absent for a counter, which has no maximum to be a proportion of.
    pub max: Option<i32>,
}

/// One value a system publishes about an actor.
///
/// Superset of [`GraphQLAttribute`]: it carries derived values too, and says
/// which is which. The older field stays because it is what the canvas and
/// the existing panels read, and moving them is not this increment's job.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLDeclaredValue {
    /// The system's own identifier — `might`, `strength`, `wishPointsForLevel`.
    pub id: String,
    pub label: String,
    pub abbreviation: Option<String>,
    /// Rendered as text, because systems do not agree on the type of a value:
    /// a score is an integer, a Fate ladder rung is a word, a proficiency is
    /// a flag. Sending each as itself would need a union the layout layer
    /// would have to branch on, and branching on a value's type is the first
    /// step towards knowing what it means.
    pub value: String,
    /// Present only for a pool. A consumer drawing a bar reads this and never
    /// parses `value`.
    pub fraction: Option<GraphQLValueFraction>,
    pub origin: GraphQLValueOrigin,
}

/// What a token's sheet says about it.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLTokenAttributes {
    pub token_id: Uuid,
    pub attributes: Vec<GraphQLAttribute>,
    /// Everything the system publishes — the attributes above plus whatever
    /// it derives — each saying where it came from.
    pub values: Vec<GraphQLDeclaredValue>,
    /// Only the movement types this creature actually has.
    ///
    /// An absent type means it cannot move that way at all, which is a
    /// different claim from a speed of zero (it can, but is currently
    /// prevented) — so absent types are omitted rather than sent as zeroes.
    pub speeds: Vec<GraphQLSpeed>,
}

#[derive(Default)]
pub struct TokenAttributesQuery;

#[Object]
impl TokenAttributesQuery {
    /// Attribute scores for every token in a scene that has any.
    ///
    /// A token with no actor, or an actor whose sheet is unfilled, is omitted
    /// rather than returned empty — an empty list would say "this character
    /// has no attributes", which is a claim about the ruleset rather than
    /// about the sheet.
    async fn token_attributes(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLTokenAttributes>> {
        let user = authenticated_user(ctx)?;
        let user_id = user.user_id;
        let is_admin = user.is_admin;
        let state = app_state(ctx)?;
        let systems_dir = state.directories.systems_dir.clone();

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        tokio::task::spawn_blocking(move || {
            use crate::schema::{scenes, tokens, world_actor_system_data, worlds};

            // Membership before anything is read. A character sheet is
            // world-scoped information about other people.
            let world_id: Uuid = scenes::table
                .filter(scenes::scene_id.eq(scene_id))
                .select(scenes::world_id)
                .first(&mut conn)
                .map_err(|_| Error::new("Scene not found"))?;

            let actor = crate::auth::world_membership::actor_in_world(
                &mut conn, user_id, is_admin, world_id,
            );
            if actor.role.is_none() && !actor.is_site_admin {
                return Err(Error::new("Not a member of this world"));
            }

            let system_id: Option<String> = worlds::table
                .filter(worlds::id.eq(world_id))
                .select(worlds::game_system_id)
                .first(&mut conn)
                .optional()
                .map_err(|e| Error::new(format!("Failed to read world: {e}")))?
                .flatten();

            let declarations: Vec<AttributeDeclaration> = match system_id.as_deref() {
                Some(id) => attribute_declarations_for_system(&systems_dir, id),
                // A world with no system has no attribute set. Not an error.
                None => Vec::new(),
            };
            let movement: Vec<MovementDeclaration> = match system_id.as_deref() {
                Some(id) => movement_declarations_for_system(&systems_dir, id),
                None => Vec::new(),
            };

            // A system may declare movement but no attributes, or the
            // reverse — Blades measures no movement at all. Only bail when
            // it declares neither.
            if declarations.is_empty() && movement.is_empty() {
                return Ok(Vec::new());
            }

            let rows: Vec<(Uuid, Option<Uuid>)> = tokens::table
                .filter(tokens::scene_id.eq(scene_id))
                .select((tokens::token_id, tokens::actor_id))
                .load(&mut conn)
                .map_err(|e| Error::new(format!("Failed to load tokens: {e}")))?;

            let actor_ids: Vec<Uuid> = rows.iter().filter_map(|(_, a)| *a).collect();

            // One read for the whole scene rather than one per token.
            // Every slot, not only `ability_data`: a system's rules read from
            // wherever their inputs live, and Genie's by-level Wish Points
            // rule reads a `level` that sits in the trait slot.
            type SlotRow = (
                Uuid,
                Option<serde_json::Value>,
                Option<serde_json::Value>,
                Option<serde_json::Value>,
                Option<serde_json::Value>,
            );
            let slots: HashMap<Uuid, ActorSlots> = world_actor_system_data::table
                .filter(world_actor_system_data::actor_id.eq_any(&actor_ids))
                .select((
                    world_actor_system_data::actor_id,
                    world_actor_system_data::ability_data,
                    world_actor_system_data::resource_data,
                    world_actor_system_data::proficiency_data,
                    world_actor_system_data::trait_data,
                ))
                .load::<SlotRow>(&mut conn)
                .unwrap_or_default()
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
                .collect();

            let mut out = Vec::new();
            for (token_id, actor_id) in rows {
                // A token bound to no actor is a marker, not a creature.
                let Some(actor_id) = actor_id else {
                    continue;
                };
                let Some(slot) = slots.get(&actor_id) else {
                    continue;
                };
                let Some(data) = slot.ability_data.as_ref() else {
                    continue;
                };

                let resolved = attributes_from(data, &declarations);
                let speeds = speeds_from(data, &movement);
                let values = match system_id.as_deref() {
                    Some(id) => declared_values_for_actor(&systems_dir, id, slot),
                    None => Vec::new(),
                };
                if resolved.is_empty() && speeds.is_empty() && values.is_empty() {
                    continue;
                }

                // Labels come from the declarations; the resolved speeds
                // carry only ids and values.
                let labelled: Vec<GraphQLSpeed> = {
                    let mut ordered: Vec<&MovementDeclaration> = movement.iter().collect();
                    ordered.sort_by_key(|d| d.order);
                    ordered
                        .into_iter()
                        .filter_map(|declaration| {
                            speeds.get(&declaration.id).map(|value| GraphQLSpeed {
                                id: declaration.id.clone(),
                                label: declaration.label.clone(),
                                value: value as f64,
                            })
                        })
                        .collect()
                };

                out.push(GraphQLTokenAttributes {
                    token_id,
                    attributes: resolved
                        .into_iter()
                        .map(|a| GraphQLAttribute {
                            id: a.id,
                            label: a.label,
                            abbreviation: a.abbreviation,
                            value: a.value,
                        })
                        .collect(),
                    values: values
                        .into_iter()
                        .map(|value| GraphQLDeclaredValue {
                            id: value.id,
                            label: value.label,
                            abbreviation: value.abbreviation,
                            value: render(&value.value),
                            fraction: fraction_of(&value.value),
                            origin: match value.origin {
                                Origin::Stored => GraphQLValueOrigin::Stored,
                                Origin::Derived => GraphQLValueOrigin::Derived,
                            },
                        })
                        .collect(),
                    speeds: labelled,
                });
            }

            Ok(out)
        })
        .await
        .map_err(|e| Error::new(format!("Task failed: {e}")))?
    }
}
