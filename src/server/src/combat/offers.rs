//! An offer of damage or healing, taken or declined once (spec 046 US2,
//! decision 1, FR-005, FR-008, FR-009).
//!
//! Nobody is told by the software that they have been hit. A hit arrives at
//! whoever controls the target as an offer; they take it and their hit points
//! change, or they decline it and nothing does. An offer waits: it survives
//! its controller going offline and is there when they come back, and it never
//! expires or resolves itself. A Game Master may take or decline it for them at
//! any time, and the table is told that the Game Master did.
//!
//! # C6: once
//!
//! The offer row is locked (`SELECT … FOR UPDATE`) and its status read under
//! the lock, so two people pressing Take together resolve it once: the second
//! waits for the first to commit, then finds it resolved and is refused.
//!
//! # Offers and relinking
//!
//! An offer is bound to its token **and to the token's link state when the
//! offer was made** (`world_offers.target_linked`). A goblin copy hit for 5
//! and then relinked to its NPC would otherwise land those 5 on the NPC's own
//! sheet — every future copy's starting hit points — though the attack was
//! rolled against a goblin with its own. So taking an offer whose token has
//! been relinked or unlinked since is **refused**, with a sentence saying so;
//! declining it still works, and the Game Master changes hit points by hand if
//! the hit should stand. The token row is locked before the comparison, as
//! damage to a copy locks it, so a relink cannot slip between the check and
//! the write.

use diesel::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

use crate::combat::attack::FightRefusal;
use crate::combat::controllers::{may_move, player_controllers, token_control};
use crate::combat::hit_points::{HitPointChangeKind, apply_hit_point_change};
use crate::combat::records::*;
use crate::play_pause::gate::refuse_if_paused;
use crate::schema::{tokens, world_offers};
use crate::world_events::{EVENT_CODE_OFFER_CHANGED, record_world_event};

/// C6's sentence.
pub const ALREADY_RESOLVED: &str = "That offer has already been resolved";

/// The relink rule's sentence.
pub const RELINKED_SINCE: &str = "That creature was relinked after this offer was made, so its hit points are a different record now. Decline the offer, and change its hit points by hand if the hit should stand.";

/// Take or decline an offer.
///
/// By a controller of the offer's token (research R7), or by a Game Master,
/// who resolves it on its controller's behalf when a player controls it.
/// Taking applies the change through `apply_hit_point_change` in the same
/// transaction as the offer's new status, so the two commit together or not
/// at all.
pub fn resolve_offer(
    conn: &mut PgConnection,
    systems_dir: &str,
    user_id: Uuid,
    is_admin: bool,
    offer_id: Uuid,
    take: bool,
) -> Result<OfferRecord, FightRefusal> {
    let not_there = || FightRefusal::NotFound("That offer is not there".to_string());
    let world_id = world_offers::table
        .filter(world_offers::id.eq(offer_id))
        .select(world_offers::world_id)
        .first::<Uuid>(conn)
        .optional()?
        .ok_or_else(not_there)?;

    // C10.
    refuse_if_paused(conn, world_id).map_err(FightRefusal::Paused)?;

    let resolved = conn.transaction::<OfferRecord, FightRefusal, _>(|conn| {
        let offer = world_offers::table
            .filter(world_offers::id.eq(offer_id))
            .select(OfferRecord::as_select())
            .for_update()
            .first::<OfferRecord>(conn)?;

        let control = token_control(conn, offer.target_token_id)?.ok_or_else(not_there)?;
        let runs_the_world =
            crate::auth::world_membership::actor_in_world(conn, user_id, is_admin, world_id)
                .runs_the_world();
        let players = player_controllers(conn, &control)?;
        // A token no player controls is the Game Masters' to resolve.
        if !runs_the_world && !may_move(conn, user_id, is_admin, &control)? {
            return Err(FightRefusal::NotControlled);
        }
        // C6: decided under the lock.
        if offer.status != OFFER_PENDING {
            return Err(FightRefusal::Invalid(ALREADY_RESOLVED.to_string()));
        }
        let on_behalf = runs_the_world && !players.is_empty() && !players.contains(&user_id);

        let status = if take {
            // The token row, locked before it is compared, as damage to a copy
            // locks it (research R5): a relink waits here, or this waits for it.
            let linked_now = tokens::table
                .filter(tokens::token_id.eq(offer.target_token_id))
                .select(tokens::linked)
                .for_update()
                .first::<bool>(conn)?;
            if linked_now != offer.target_linked {
                return Err(FightRefusal::Invalid(RELINKED_SINCE.to_string()));
            }
            let kind = if offer.kind == OFFER_HEALING {
                HitPointChangeKind::Healing
            } else {
                HitPointChangeKind::Damage
            };
            apply_hit_point_change(
                conn,
                systems_dir,
                offer.target_token_id,
                kind,
                offer.amount,
                user_id,
            )
            .map_err(FightRefusal::Invalid)?;
            OFFER_TAKEN
        } else {
            OFFER_DECLINED
        };

        let now = chrono::Utc::now().naive_utc();
        let updated = diesel::update(world_offers::table.filter(world_offers::id.eq(offer_id)))
            .set((
                world_offers::status.eq(status),
                world_offers::resolved_by.eq(Some(user_id)),
                world_offers::resolved_on_behalf.eq(on_behalf),
                world_offers::resolved_at.eq(Some(now)),
                world_offers::updated_by.eq(user_id),
                world_offers::updated_at.eq(now),
            ))
            .returning(OfferRecord::as_returning())
            .get_result::<OfferRecord>(conn)?;

        let _ = record_world_event(
            conn,
            world_id,
            EVENT_CODE_OFFER_CHANGED,
            Some(serde_json::json!({ "offerId": offer_id })),
            user_id,
        );
        Ok(updated)
    })?;

    Ok(resolved)
}

/// Whether `user_id` may resolve this offer: a controller of its token, or a
/// Game Master of its world.
pub fn may_resolve(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    offer: &OfferRecord,
) -> QueryResult<bool> {
    if crate::auth::world_membership::actor_in_world(conn, user_id, is_admin, offer.world_id)
        .runs_the_world()
    {
        return Ok(true);
    }
    match token_control(conn, offer.target_token_id)? {
        Some(control) => may_move(conn, user_id, is_admin, &control),
        None => Ok(false),
    }
}

/// The pending offers `user_id` may resolve, oldest first (FR-008).
///
/// A Game Master receives every pending offer in the world, since they may
/// resolve any of them (FR-009). A player receives only those against a
/// creature they control — never one against somebody else's, which would
/// tell them what was hit.
pub fn pending_offers(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> QueryResult<Vec<OfferRecord>> {
    let pending = world_offers::table
        .filter(world_offers::world_id.eq(world_id))
        .filter(world_offers::status.eq(OFFER_PENDING))
        .order((world_offers::created_at, world_offers::id))
        .select(OfferRecord::as_select())
        .load::<OfferRecord>(conn)?;
    let runs_the_world =
        crate::auth::world_membership::actor_in_world(conn, user_id, is_admin, world_id)
            .runs_the_world();
    if runs_the_world {
        return Ok(pending);
    }
    let mut mine = Vec::new();
    let mut decided: std::collections::HashMap<Uuid, bool> = std::collections::HashMap::new();
    for offer in pending {
        let controls = match decided.get(&offer.target_token_id) {
            Some(answer) => *answer,
            None => {
                let answer = may_resolve(conn, user_id, is_admin, &offer)?;
                decided.insert(offer.target_token_id, answer);
                answer
            }
        };
        if controls {
            mine.push(offer);
        }
    }
    Ok(mine)
}

#[cfg(test)]
#[path = "offers_tests.rs"]
mod tests;
