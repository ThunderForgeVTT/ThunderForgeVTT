//! `makeAttack`, `resolveOffer`, the auto-apply settings, and what an ability
//! or item is as an attack (spec 046 contracts/fight.md §1, ADR-101).
//!
//! The rules are `crate::combat`'s; this file is who may reach them and what
//! they are told. Every mutation here refuses while the world's play is paused
//! (C10), and each is listed in `play_pause_surface_tables.rs`.

use async_graphql::{Context, Error, InputObject, Object, Result as GraphQLResult};
use diesel::prelude::*;
use rand::SeedableRng;
use uuid::Uuid;

use crate::auth::world_membership::is_dm_of_world;
use crate::combat::records::{AttackRecord, OfferRecord};
use crate::graphql::mutations_combat::GraphQLCombat;
use crate::graphql::types::GraphQLWorld;
use crate::graphql::types::types_attacks::{
    AttackFieldsInput, AttackInput, GraphQLAttack, GraphQLOffer, Sights, build_attacks,
    build_offers,
};
use crate::graphql::{app_state, authenticated_user};
use crate::models::World;
use crate::play_pause::gate::{refusal_or, refuse_if_paused};
use crate::schema::{world_abilities, world_attacks, world_combats, world_items, worlds};
use crate::state::AppState;

/// Testable core of `makeAttack`, with the RNG injected.
pub async fn make_attack_impl<R: rand::Rng + Send + 'static>(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: AttackInput,
    mut rng: R,
) -> GraphQLResult<Vec<GraphQLAttack>> {
    let systems_dir = state.directories.systems_dir.clone();
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || {
        let made = crate::combat::attack::make_attack(
            &mut conn,
            &systems_dir,
            user_id,
            is_admin,
            &input.into_request()?,
            &mut rng,
        )?;
        let records = world_attacks::table
            .filter(world_attacks::id.eq_any(&made.attack_ids))
            .select(AttackRecord::as_select())
            .load::<AttackRecord>(&mut conn)
            .map_err(|_| Error::new("Failed to load attack"))?;
        let mut ordered = Vec::with_capacity(records.len());
        for id in &made.attack_ids {
            if let Some(record) = records.iter().find(|r| r.id == *id) {
                ordered.push(record.clone());
            }
        }
        let mut sights = Sights::new(&systems_dir, user_id, is_admin);
        build_attacks(&mut conn, &mut sights, ordered)
            .map_err(|e| Error::new(format!("Failed to read attack: {e}")))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

pub async fn resolve_offer_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    offer_id: Uuid,
    take: bool,
) -> GraphQLResult<GraphQLOffer> {
    let systems_dir = state.directories.systems_dir.clone();
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || {
        let offer: OfferRecord = crate::combat::offers::resolve_offer(
            &mut conn,
            &systems_dir,
            user_id,
            is_admin,
            offer_id,
            take,
        )?;
        let mut sights = Sights::new(&systems_dir, user_id, is_admin);
        build_offers(&mut conn, &mut sights, vec![offer])
            .map_err(|e| Error::new(format!("Failed to read offer: {e}")))?
            .pop()
            .ok_or_else(|| Error::new("Failed to read offer"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

#[derive(InputObject, Debug, Clone)]
pub struct UpdateWorldAutoApplyNpcDamageInput {
    pub world_id: Uuid,
    pub enabled: bool,
}

/// The world's auto-apply default (FR-006). Game Master only.
pub async fn update_world_auto_apply_npc_damage_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: UpdateWorldAutoApplyNpcDamageInput,
) -> GraphQLResult<GraphQLWorld> {
    if !is_dm_of_world(state, user_id, is_admin, input.world_id).await? {
        return Err(Error::new(
            "Only the Game Master may change whether damage to their NPCs is applied automatically",
        ));
    }
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = input.world_id;
    let enabled = input.enabled;
    let updated = tokio::task::spawn_blocking(move || {
        refuse_if_paused(&mut conn, world_id)?;
        diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::auto_apply_npc_damage.eq(enabled))
            .returning(World::as_returning())
            .get_result::<World>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| refusal_or(e, "Failed to update the auto-apply setting"))?;
    Ok(GraphQLWorld::from(updated))
}

/// This encounter's auto-apply override; `None` goes back to the world's.
/// Game Master only.
pub async fn set_combat_auto_apply_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    combat_id: Uuid,
    enabled: Option<bool>,
) -> GraphQLResult<GraphQLCombat> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = tokio::task::spawn_blocking(move || {
        world_combats::table
            .filter(world_combats::id.eq(combat_id))
            .select(world_combats::world_id)
            .first::<Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load combat"))?
    .ok_or_else(|| Error::new("Combat not found"))?;

    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new(
            "Only the Game Master may change auto-apply for an encounter",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    refuse_if_paused(&mut conn, world_id)?;
    let systems_dir = state.directories.systems_dir.clone();
    tokio::task::spawn_blocking(move || -> Result<GraphQLCombat, String> {
        use crate::graphql::mutations_combat::{combat_world, load_combat, touch_and_broadcast};
        diesel::update(world_combats::table.filter(world_combats::id.eq(combat_id)))
            .set(world_combats::auto_apply.eq(enabled))
            .execute(&mut conn)
            .map_err(|e| format!("Failed to update combat: {e}"))?;
        touch_and_broadcast(&mut conn, combat_id, world_id, user_id)?;
        let combat = combat_world(&mut conn, combat_id)?;
        load_combat(&mut conn, &systems_dir, combat, user_id, is_admin)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Which content row `set_attack_fields_impl` writes.
#[derive(Clone, Copy, Debug)]
pub enum AttackFieldsOwner {
    Ability(Uuid),
    Item(Uuid),
}

/// What an ability or item is as an attack (research R2). Requires Editor,
/// as editing the rest of it does.
pub async fn set_attack_fields_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    owner: AttackFieldsOwner,
    input: AttackFieldsInput,
) -> GraphQLResult<bool> {
    use crate::graphql::types::ActorPermissionLevel;
    input.check().map_err(Error::new)?;
    let world_id = match owner {
        AttackFieldsOwner::Ability(id) => {
            crate::auth::ability_permissions::require_ability_permission(
                state,
                user_id,
                is_admin,
                id,
                ActorPermissionLevel::Editor,
            )
            .await?;
            let mut conn = state
                .db_pool
                .get()
                .map_err(|_| Error::new("Failed to get DB connection"))?;
            world_abilities::table
                .filter(world_abilities::id.eq(id))
                .select(world_abilities::world_id)
                .first::<Uuid>(&mut conn)
                .map_err(|_| Error::new("Ability not found"))?
        }
        AttackFieldsOwner::Item(id) => {
            crate::auth::item_permissions::require_item_permission(
                state,
                user_id,
                is_admin,
                id,
                ActorPermissionLevel::Editor,
            )
            .await?;
            let mut conn = state
                .db_pool
                .get()
                .map_err(|_| Error::new("Failed to get DB connection"))?;
            world_items::table
                .filter(world_items::id.eq(id))
                .select(world_items::world_id)
                .first::<Uuid>(&mut conn)
                .map_err(|_| Error::new("Item not found"))?
        }
    };

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || -> GraphQLResult<bool> {
        refuse_if_paused(&mut conn, world_id)?;
        // A multiattack names abilities of the same world, and never itself.
        let named: Vec<Uuid> = input.multiattack.clone();
        if !named.is_empty() {
            let found = world_abilities::table
                .filter(world_abilities::id.eq_any(&named))
                .filter(world_abilities::world_id.eq(world_id))
                .count()
                .get_result::<i64>(&mut conn)
                .map_err(|_| Error::new("Failed to check the multiattack"))?;
            let distinct: std::collections::HashSet<&Uuid> = named.iter().collect();
            if found as usize != distinct.len() {
                return Err(Error::new(
                    "A multiattack can only name abilities in this world",
                ));
            }
            if let AttackFieldsOwner::Ability(id) = owner
                && named.contains(&id)
            {
                return Err(Error::new("A multiattack cannot name itself"));
            }
        }
        let multiattack: Vec<Option<Uuid>> = named.into_iter().map(Some).collect();
        let cost = input.action_cost.as_db_str();
        let now = chrono::Utc::now().naive_utc();
        let written = match owner {
            AttackFieldsOwner::Ability(id) => {
                diesel::update(world_abilities::table.filter(world_abilities::id.eq(id)))
                    .set((
                        world_abilities::reach.eq(input.reach),
                        world_abilities::range_normal.eq(input.range_normal),
                        world_abilities::range_long.eq(input.range_long),
                        world_abilities::needs_line_of_sight.eq(input.needs_line_of_sight),
                        world_abilities::action_cost.eq(cost),
                        world_abilities::legendary_cost.eq(input.legendary_cost),
                        world_abilities::multiattack.eq(&multiattack),
                        world_abilities::updated_by.eq(user_id),
                        world_abilities::updated_at.eq(now),
                    ))
                    .execute(&mut conn)
            }
            AttackFieldsOwner::Item(id) => {
                diesel::update(world_items::table.filter(world_items::id.eq(id)))
                    .set((
                        world_items::reach.eq(input.reach),
                        world_items::range_normal.eq(input.range_normal),
                        world_items::range_long.eq(input.range_long),
                        world_items::needs_line_of_sight.eq(input.needs_line_of_sight),
                        world_items::action_cost.eq(cost),
                        world_items::legendary_cost.eq(input.legendary_cost),
                        world_items::multiattack.eq(&multiattack),
                        world_items::updated_at.eq(now),
                    ))
                    .execute(&mut conn)
            }
        }
        .map_err(|e| Error::new(format!("Failed to save the attack: {e}")))?;
        Ok(written == 1)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

#[derive(Default)]
pub struct AttackMutation;

#[Object]
impl AttackMutation {
    /// Make an attack against a target, or at nothing (spec 046 US1).
    ///
    /// Rolled and judged on the server. A player is refused on somebody
    /// else's turn unless it is a reaction; a hit's damage is offered to
    /// whoever controls the target. Several attacks for a multiattack.
    async fn make_attack(
        &self,
        ctx: &Context<'_>,
        input: AttackInput,
    ) -> GraphQLResult<Vec<GraphQLAttack>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        // The same RNG `rollDice` uses, for the same reason (ADR-044).
        let rng = rand::rngs::StdRng::from_rng(&mut rand::rng());
        make_attack_impl(state, user.user_id, user.is_admin, input, rng).await
    }

    /// Take or decline an offer, once. By a controller of its creature, or a
    /// Game Master on their behalf.
    async fn resolve_offer(
        &self,
        ctx: &Context<'_>,
        offer_id: Uuid,
        take: bool,
    ) -> GraphQLResult<GraphQLOffer> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        resolve_offer_impl(state, user.user_id, user.is_admin, offer_id, take).await
    }

    /// Whether hits on NPCs the Game Master runs are applied without an
    /// offer, by default, in this world. Off unless switched on.
    async fn update_world_auto_apply_npc_damage(
        &self,
        ctx: &Context<'_>,
        input: UpdateWorldAutoApplyNpcDamageInput,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        update_world_auto_apply_npc_damage_impl(state, user.user_id, user.is_admin, input).await
    }

    /// Override auto-apply for one encounter; null returns to the world's.
    async fn set_combat_auto_apply(
        &self,
        ctx: &Context<'_>,
        combat_id: Uuid,
        enabled: Option<bool>,
    ) -> GraphQLResult<GraphQLCombat> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        set_combat_auto_apply_impl(state, user.user_id, user.is_admin, combat_id, enabled).await
    }

    /// What an ability is as an attack: reach, range, line of sight, cost and
    /// multiattack.
    async fn set_ability_attack(
        &self,
        ctx: &Context<'_>,
        ability_id: Uuid,
        attack: AttackFieldsInput,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        set_attack_fields_impl(
            state,
            user.user_id,
            user.is_admin,
            AttackFieldsOwner::Ability(ability_id),
            attack,
        )
        .await
    }

    /// What an item is as an attack.
    async fn set_item_attack(
        &self,
        ctx: &Context<'_>,
        item_id: Uuid,
        attack: AttackFieldsInput,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        set_attack_fields_impl(
            state,
            user.user_id,
            user.is_admin,
            AttackFieldsOwner::Item(item_id),
            attack,
        )
        .await
    }
}

#[cfg(test)]
#[path = "mutations_attacks_tests.rs"]
mod tests;
