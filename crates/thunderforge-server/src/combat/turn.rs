//! Whose turn it is, asked before a player acts (spec 046 research R12, C1).
//!
//! The tracker has pointed at a combatant since spec 031, and nothing read the
//! pointer: a player moved on the ogre's turn and the product let them. This
//! is the one question every player path asks — `moveOwnToken`, a queued
//! offline move, and (with attacks) `makeAttack` — so the answer, and the
//! sentence a refusal is given in, cannot differ between them.
//!
//! # Who is held, and who is not
//!
//! Refused only when all of these hold:
//!
//! - a combat is running in the token's world, and it is this scene's combat
//!   (a combat with no scene is the world's, and holds every scene);
//! - the acting token is in it (a player exploring elsewhere on the scene is
//!   not in the fight);
//! - the turn has started (a combat whose first turn nobody has taken yet
//!   holds nobody to an order that has not begun);
//! - it is somebody else's turn;
//! - the caller does not run the world. A Game Master is never refused.
//!
//! Only turn order refuses (clarification 1). Reach, range and budgets are
//! flags on what was done, never reasons not to do it.
//!
//! # The label
//!
//! "It is <label>'s turn", where the label follows the tracker's own rule
//! (`load_combat`): a combatant whose token's name the Game Master hid reads
//! "Unknown" to a player, so a refusal cannot be used to learn a hidden name.

use diesel::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

use crate::models::Combatant;
use crate::schema::{scenes, tokens, world_combatants, world_combats};

/// What a player's attempt to act would meet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnCheck {
    pub allowed: bool,
    /// Whose turn it is, when the answer is no. Already redacted for the
    /// caller.
    pub active_label: Option<String>,
}

impl TurnCheck {
    fn permitted() -> Self {
        TurnCheck {
            allowed: true,
            active_label: None,
        }
    }

    /// The sentence a refusal is given in (C1), or `None` when allowed.
    pub fn refusal(&self) -> Option<String> {
        if self.allowed {
            return None;
        }
        Some(turn_refusal(self.active_label.as_deref().unwrap_or(
            crate::graphql::mutations_combat::UNKNOWN_COMBATANT,
        )))
    }
}

/// "It is <label>'s turn", and who holds a token back, are the shared rules'
/// (`thunderforge_combat::turn`, ADR-113): the browser demo refuses in the
/// same words. This module finds the combat and redacts the label.
pub use thunderforge_combat::turn::turn_refusal;

/// A combatant row, as the turn check reads it.
impl thunderforge_combat::turn::Party for Combatant {
    type Id = Uuid;
    fn party_id(&self) -> Uuid {
        self.id
    }
    fn token(&self) -> Option<Uuid> {
        self.token_id
    }
    fn actor(&self) -> Option<Uuid> {
        self.actor_id
    }
}

/// The world's running combat in this scene (a combat with no scene is the
/// world's, and holds every scene), and its auto-apply override.
pub fn running_combat(
    conn: &mut PgConnection,
    world_id: Uuid,
    scene_id: Uuid,
) -> QueryResult<Option<(Uuid, Option<bool>)>> {
    world_combats::table
        .filter(world_combats::world_id.eq(world_id))
        .filter(world_combats::ended_at.is_null())
        .filter(
            world_combats::scene_id
                .is_null()
                .or(world_combats::scene_id.eq(scene_id)),
        )
        .select((world_combats::id, world_combats::auto_apply))
        .first::<(Uuid, Option<bool>)>(conn)
        .optional()
}

/// Whether `user_id` may act with `acting_token_id` now.
///
/// `scene_id` is the scene the token is on. A token that does not exist is
/// allowed here: whoever called has its own answer for a token that is not
/// there, and it is not "it is somebody else's turn".
pub fn turn_check(
    conn: &mut PgConnection,
    scene_id: Uuid,
    acting_token_id: Uuid,
    user_id: Uuid,
    is_admin: bool,
) -> QueryResult<TurnCheck> {
    let Some(world_id) = scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .select(scenes::world_id)
        .first::<Uuid>(conn)
        .optional()?
    else {
        return Ok(TurnCheck::permitted());
    };

    let caller = crate::auth::world_membership::actor_in_world(conn, user_id, is_admin, world_id);
    if caller.runs_the_world() {
        return Ok(TurnCheck::permitted());
    }

    let combat = world_combats::table
        .filter(world_combats::world_id.eq(world_id))
        .filter(world_combats::ended_at.is_null())
        .filter(
            world_combats::scene_id
                .is_null()
                .or(world_combats::scene_id.eq(scene_id)),
        )
        .select((world_combats::id, world_combats::active_combatant_id))
        .first::<(Uuid, Option<Uuid>)>(conn)
        .optional()?;
    let Some((combat_id, Some(active_id))) = combat else {
        return Ok(TurnCheck::permitted());
    };

    let Some(actor_id) = tokens::table
        .filter(tokens::token_id.eq(acting_token_id))
        .select(tokens::actor_id)
        .first::<Option<Uuid>>(conn)
        .optional()?
    else {
        return Ok(TurnCheck::permitted());
    };

    let combatants = world_combatants::table
        .filter(world_combatants::combat_id.eq(combat_id))
        .select(Combatant::as_select())
        .load::<Combatant>(conn)?;

    // The acting token's own combatant rows are found by token, or by actor
    // for a combatant added from the actor list with no token of its own; a
    // pointer naming a combatant that is gone holds nobody to it.
    let Some(active) =
        thunderforge_combat::turn::held_by(&combatants, active_id, acting_token_id, actor_id)
    else {
        return Ok(TurnCheck::permitted());
    };

    // The one rule (owner decision 2026-09-15): the token's own switch, and
    // the creature it stands for being one players may see.
    let hidden = match active.token_id {
        Some(token) => !crate::auth::npc_visibility::token_name_readable_sync(conn, token)?,
        None => false,
    };
    let label = if hidden {
        crate::graphql::mutations_combat::UNKNOWN_COMBATANT.to_string()
    } else {
        active.label.clone()
    };

    Ok(TurnCheck {
        allowed: false,
        active_label: Some(label),
    })
}

#[cfg(test)]
#[path = "turn_tests.rs"]
pub(crate) mod tests;
