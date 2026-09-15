//! A lair (spec 046 US6, FR-053, research R14).
//!
//! A lair is a combatant that is not a creature (`kind = 'lair'`): no token,
//! no actor, at initiative count 20, losing ties (tiebreak −1). The Game
//! Master adds it from the tracker, and acts for it when its count comes round.
//!
//! # A lair's action
//!
//! Made through `makeAttack` with `lairCombatantId` in place of an attacker
//! token, and resolved like any other attack (FR-051): the same roll, defence,
//! outcome, offer and auto-apply, and the same record. What differs follows
//! from its having no body:
//!
//! - **Control (C2)**: only a Game Master acts for a lair. A player is refused
//!   with C2's sentence; there is no controller to be.
//! - **Turn (C1)**: a Game Master is never held to the turn, so neither is a
//!   lair. It may act on any count; its place at 20 is where the table expects
//!   it, not a gate.
//! - **Reach, range and line of sight (C3)**: not measured. There is no
//!   footprint to measure from, so the attack has no `distance` and none of
//!   those flags; and so auto-apply never waits on line of sight for it.
//! - **Budget (C9)**: none. A lair has no budget (`budget::budgets_for`), so
//!   nothing is spent and it is never flagged `overspent`.
//! - **Redaction (§3)**: a lair's action is recorded with `attacker_kind =
//!   'lair'` and its label, and every seat is told the label and what it used.
//!   Nothing of a lair can be hidden from a board — the tracker already shows
//!   its name to everyone. The **target** is redacted as for any attack.
//! - **Scene**: the encounter's scene; for an encounter held world-wide, the
//!   target's. A world-wide lair action at nothing has no scene to be in, and
//!   is refused as such.

use diesel::PgConnection;
use diesel::prelude::*;
use rand::Rng;
use uuid::Uuid;

use crate::combat::attack::{
    AttackPreview, AttackRequest, FightRefusal, MadeAttack, Settled, record_attack, target_of,
    targets_on_scene,
};
use crate::combat::reach::{Measured, Reach};
use crate::combat::records::KIND_LAIR;
use crate::combat::turn::TurnCheck;
use crate::combat::weapon::{find_weapon, parts_of};
use crate::models::{Combat, Combatant};
use crate::play_pause::gate::refuse_if_paused;
use crate::schema::{tokens, world_combatants, world_combats};

/// A lair's initiative count (FR-053).
pub const LAIR_INITIATIVE: i32 = 20;
/// A lair loses ties: any creature at 20 goes first.
pub const LAIR_TIEBREAK: i32 = -1;

/// The lair and its encounter, which must still be running.
pub fn lair_in_fight(
    conn: &mut PgConnection,
    combatant_id: Uuid,
) -> Result<(Combatant, Combat), FightRefusal> {
    let not_there =
        || FightRefusal::NotFound("That lair is not in a running encounter".to_string());
    let lair = world_combatants::table
        .filter(world_combatants::id.eq(combatant_id))
        .filter(world_combatants::kind.eq(KIND_LAIR))
        .select(Combatant::as_select())
        .first::<Combatant>(conn)
        .optional()?
        .ok_or_else(not_there)?;
    let combat = world_combats::table
        .filter(world_combats::id.eq(lair.combat_id))
        .filter(world_combats::ended_at.is_null())
        .select(Combat::as_select())
        .first::<Combat>(conn)
        .optional()?
        .ok_or_else(not_there)?;
    Ok((lair, combat))
}

fn runs_the_world(conn: &mut PgConnection, user_id: Uuid, is_admin: bool, world_id: Uuid) -> bool {
    crate::auth::world_membership::actor_in_world(conn, user_id, is_admin, world_id)
        .runs_the_world()
}

/// What `previewAttack` answers for a lair: allowed, and nothing measured.
pub fn preview_lair_action(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    combatant_id: Uuid,
) -> Result<AttackPreview, FightRefusal> {
    let (_, combat) = lair_in_fight(conn, combatant_id)?;
    if !runs_the_world(conn, user_id, is_admin, combat.world_id) {
        return Err(FightRefusal::NotControlled);
    }
    Ok(AttackPreview {
        distance: None,
        flags: Vec::new(),
        turn: TurnCheck {
            allowed: true,
            active_label: None,
        },
        reach: Reach::default(),
        unit_label: String::new(),
    })
}

/// Make a lair's action. See the module documentation.
pub fn make_lair_action<R: Rng>(
    conn: &mut PgConnection,
    systems_dir: &str,
    user_id: Uuid,
    is_admin: bool,
    combatant_id: Uuid,
    request: &AttackRequest,
    rng: &mut R,
) -> Result<MadeAttack, FightRefusal> {
    let (lair, combat) = lair_in_fight(conn, combatant_id)?;
    let world_id = combat.world_id;

    // C10.
    refuse_if_paused(conn, world_id).map_err(FightRefusal::Paused)?;
    // C2: nobody controls a lair; the Game Master acts for it.
    if !runs_the_world(conn, user_id, is_admin, world_id) {
        return Err(FightRefusal::NotControlled);
    }

    let scene_id = match combat.scene_id {
        Some(scene_id) => scene_id,
        None => {
            let target = target_of(request, 0).ok_or_else(|| {
                FightRefusal::Invalid(
                    "A lair's action needs a target when its encounter is not in one scene"
                        .to_string(),
                )
            })?;
            tokens::table
                .filter(tokens::token_id.eq(target))
                .select(tokens::scene_id)
                .first::<Uuid>(conn)
                .optional()?
                .ok_or_else(|| {
                    FightRefusal::NotFound("That target is not on the board".to_string())
                })?
        }
    };

    let weapon = find_weapon(
        conn,
        world_id,
        None,
        true,
        request.ability_id,
        request.item_id,
    )?;
    let action_cost = request.action_cost.unwrap_or_else(|| weapon.action_cost());
    let parts = parts_of(conn, world_id, &weapon)?;
    targets_on_scene(conn, request, parts.len(), scene_id)?;
    let measured = vec![Measured::default(); parts.len()];

    record_attack(
        conn,
        systems_dir,
        user_id,
        request,
        Settled {
            world_id,
            scene_id,
            attacker_token_id: None,
            attacker_kind: KIND_LAIR,
            attacker_label: lair.label,
            measured,
            action_cost,
            legendary_cost: weapon.legendary_cost(),
            parts,
        },
        rng,
    )
}
