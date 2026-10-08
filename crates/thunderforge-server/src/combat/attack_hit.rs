//! What a hit does after it is judged (spec 046 steps 7 and 8, research R7 of
//! spec 084): roll the damage, then offer it to whoever controls the target,
//! or apply it when auto-apply holds.
//!
//! `record_attack` and the reroll of a missed attack (`combat::attack_reroll`)
//! both settle a hit here, so a hit that a reroll makes deals its damage
//! exactly as a first-time hit does.
//!
//! The damage is rolled before the attack row is written, because the row
//! names the damage roll; the offer comes after, because it names the row.
//! So this is two calls, in that order, with the row inserted between them.

use diesel::PgConnection;
use diesel::prelude::*;
use rand::Rng;
use thunderforge_canvas_core::roll_facets::{Advantage, RollKind, ShapeInput};
use thunderforge_combat::attack::{damage_source, offered_amount};
use thunderforge_dice::{PlaceholderBindings, RollResolution};
use uuid::Uuid;

use crate::combat::attack::{FightRefusal, auto_apply_holds, roll_and_record};
use crate::combat::controllers::{player_controllers, token_control};
use crate::combat::hit_points::{HitPointChangeKind, apply_hit_point_change};
use crate::combat::manifest::combat_for_system;
use crate::combat::records::*;
use crate::combat::weapon::Part;
use crate::rolls::facets::{RollMeta, shape_roll};
use crate::schema::{tokens, world_actors, world_offers};
use crate::world_events::{EVENT_CODE_OFFER_CHANGED, record_world_event};

/// Where a hit lands and what the table has said about applying damage.
pub(crate) struct HitContext<'a> {
    pub world_id: Uuid,
    pub scene_id: Uuid,
    pub world_system: Option<&'a str>,
    /// The encounter's auto-apply override, else the world's setting.
    pub auto_apply: bool,
    pub now: chrono::NaiveDateTime,
}

/// What shapes a hit's damage (spec 084 research R6): the world's system,
/// the attacker's sheet, and whether the attack was made in melee.
pub(crate) struct DamageShape<'a> {
    pub system_id: &'a str,
    pub trait_data: &'a serde_json::Value,
    pub melee: bool,
}

/// A hit's damage roll, shaped by the attacker's pack and recorded like any
/// roll as `damage`, or `None` when the part has nothing to roll.
#[allow(clippy::too_many_arguments)]
pub(crate) fn roll_hit_damage<R: Rng>(
    conn: &mut PgConnection,
    world_id: Uuid,
    user_id: Uuid,
    part: &Part,
    bindings: &PlaceholderBindings,
    shape: DamageShape<'_>,
    mut meta: RollMeta,
    rng: &mut R,
) -> Result<Option<(Uuid, RollResolution, f64)>, FightRefusal> {
    let Some(source) = damage_source(&part.damage) else {
        return Ok(None);
    };
    let shaped = shape_roll(
        shape.system_id,
        ShapeInput {
            kind: RollKind::Damage,
            formula: &source,
            trait_data: shape.trait_data,
            advantage: Advantage::Normal,
            melee: shape.melee,
            item_properties: &part.properties,
        },
    )
    .map_err(FightRefusal::Invalid)?;
    meta.roll_kind = Some(RollKind::Damage);
    meta.facets = shaped.facets;
    roll_and_record(
        conn,
        world_id,
        user_id,
        &shaped.formula,
        &format!("{} damage", part.name),
        bindings,
        meta,
        rng,
    )
    .map(Some)
}

/// The damage a hit rolled, offered to whoever controls `target` (C5), or
/// applied when auto-apply holds, and event 30. `None` when the target's
/// pack declares no hit points (M1): the number is shown and nothing offered.
#[allow(clippy::too_many_arguments)]
pub(crate) fn settle_hit(
    conn: &mut PgConnection,
    systems_dir: &str,
    user_id: Uuid,
    hit: &HitContext<'_>,
    part: &Part,
    target: Uuid,
    attack_id: Uuid,
    amount: f64,
    flags: &[String],
) -> Result<Option<Uuid>, FightRefusal> {
    let HitContext {
        world_id,
        scene_id,
        world_system,
        auto_apply,
        now,
    } = *hit;
    let target_system = target_system(conn, world_system, target)?;
    // M1: a pack with no hit points shows the number and offers nothing.
    let declares_hit_points = target_system
        .as_deref()
        .is_some_and(|id| combat_for_system(systems_dir, id).hit_points.is_some());
    if !declares_hit_points {
        return Ok(None);
    }

    let target_control = token_control(conn, target)?
        .ok_or_else(|| FightRefusal::NotFound("That target is not on this scene".to_string()))?;
    let target_linked = tokens::table
        .filter(tokens::token_id.eq(target))
        .select(tokens::linked)
        .first::<bool>(conn)?;
    let amount = offered_amount(amount);
    let offer_id = Uuid::now_v7();
    diesel::insert_into(world_offers::table)
        .values(&OfferRecord {
            id: offer_id,
            world_id,
            scene_id,
            attack_id: Some(attack_id),
            target_token_id: target,
            target_linked,
            kind: OFFER_DAMAGE.to_string(),
            amount,
            status: OFFER_PENDING.to_string(),
            resolved_by: None,
            resolved_on_behalf: false,
            resolved_at: None,
            created_by: user_id,
            updated_by: user_id,
            created_at: now,
            updated_at: now,
        })
        .execute(conn)?;

    // C5: auto-apply, or the offer waits for whoever controls the target.
    let players = player_controllers(conn, &target_control)?;
    if auto_apply_holds(auto_apply, &players, flags, part.reach.needs_line_of_sight) {
        // A savepoint: a creature with no hit points recorded leaves the
        // offer pending for a person to deal with, rather than losing the
        // attack.
        let applied = conn.transaction::<_, FightRefusal, _>(|conn| {
            apply_hit_point_change(
                conn,
                systems_dir,
                target,
                HitPointChangeKind::Damage,
                amount,
                user_id,
            )
            .map_err(FightRefusal::Invalid)?;
            diesel::update(world_offers::table.filter(world_offers::id.eq(offer_id)))
                .set((
                    world_offers::status.eq(OFFER_APPLIED),
                    world_offers::resolved_at.eq(Some(now)),
                ))
                .execute(conn)?;
            Ok(())
        });
        if let Err(FightRefusal::Invalid(reason)) = &applied {
            tracing::info!(offer_id = %offer_id, %reason, "auto-apply left an offer pending");
        } else {
            applied?;
        }
    }
    let _ = record_world_event(
        conn,
        world_id,
        EVENT_CODE_OFFER_CHANGED,
        Some(serde_json::json!({ "offerId": offer_id })),
        user_id,
    );
    Ok(Some(offer_id))
}

/// The system a target's hit points are declared by: its actor's, else the
/// world's — the same resolution `apply_hit_point_change` makes.
fn target_system(
    conn: &mut PgConnection,
    world_system: Option<&str>,
    target: Uuid,
) -> QueryResult<Option<String>> {
    let actor_system = tokens::table
        .inner_join(world_actors::table.on(world_actors::id.nullable().eq(tokens::actor_id)))
        .filter(tokens::token_id.eq(target))
        .select(world_actors::game_system_id)
        .first::<Option<String>>(conn)
        .optional()?
        .flatten();
    Ok(actor_system.or_else(|| world_system.map(str::to_string)))
}
