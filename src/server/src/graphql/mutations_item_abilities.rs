//! Abilities an item carries, and the binding facet that decides which may.
//!
//! Spec 033 FR-018 to FR-020. The item peer of `mutations_actor_abilities`,
//! following its shape deliberately — attach, detach, list, tombstones — so
//! that "an ability is attached to a thing" has one behaviour rather than two.
//!
//! # The refusal
//!
//! A type declares exactly one subject: a character, an item, or nothing
//! (FR-018 as clarified). FR-019 requires that refusal at the data boundary
//! rather than only in the interface, and SC-011 measures it "including
//! attempts that bypass the interface" — so it lives here, in the mutation,
//! not in a disabled button.

use async_graphql::{Context, Error, InputObject, Object, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use uuid::Uuid;

use crate::ability_vocabulary::{Binds, for_system};
use crate::auth::world_membership::{is_dm_of_world, require_world_member};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{NewItemAbility, WorldAbility};
use crate::schema::{world_abilities, world_item_abilities, world_items, worlds};

/// One ability an item carries, as a sheet needs it.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLItemAbilityEntry {
    pub id: Uuid,
    /// `null` once the ability itself is deleted — the row survives as a
    /// tombstone so the item does not silently lose a line.
    pub ability_id: Option<Uuid>,
    pub ability_name: String,
    /// The stored type identity, or `null` for a tombstone.
    pub classification: Option<String>,
    pub grade: Option<i32>,
}

#[derive(InputObject, Debug, Clone)]
pub struct AttachAbilityToItemInput {
    pub item_id: Uuid,
    pub ability_id: Uuid,
}

/// The world an item belongs to, and the ability's own world.
///
/// Both, because an ability may only be attached to an item in the same world
/// — attaching across worlds would make one world's content reachable from
/// another, which FR-039 forbids outright.
fn worlds_of(
    conn: &mut PgConnection,
    item_id: Uuid,
    ability_id: Uuid,
) -> Result<(Uuid, WorldAbility), diesel::result::Error> {
    let item_world: Uuid = world_items::table
        .filter(world_items::id.eq(item_id))
        .select(world_items::world_id)
        .first(conn)?;

    let ability: WorldAbility = world_abilities::table
        .filter(world_abilities::id.eq(ability_id))
        .select(WorldAbility::as_select())
        .first(conn)?;

    Ok((item_world, ability))
}

/// Testable core of `attachAbilityToItem`.
pub async fn attach_ability_to_item_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: AttachAbilityToItemInput,
) -> GraphQLResult<GraphQLItemAbilityEntry> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let item_id = input.item_id;
    let ability_id = input.ability_id;

    let (world_id, ability) =
        tokio::task::spawn_blocking(move || worlds_of(&mut conn, item_id, ability_id))
            .await
            .map_err(|_| Error::new("Failed to spawn blocking task"))?
            .map_err(|_| Error::new("Item or ability not found"))?;

    if ability.world_id != world_id {
        return Err(Error::new(
            "An ability can only be attached to an item in its own world",
        ));
    }

    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new(
            "Only the DM (Owner or GM) may attach abilities to items",
        ));
    }

    // FR-019: the binding facet decides, server-side. A type that binds to a
    // character is refused here even when the caller skipped the interface
    // entirely, which is what SC-011 measures.
    let systems_dir = state.directories.systems_dir.clone();
    let classification = ability.classification.clone();
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let binds = {
        let wanted = classification.clone();
        tokio::task::spawn_blocking(move || {
            let system_id: Option<String> = worlds::table
                .filter(worlds::id.eq(world_id))
                .select(worlds::game_system_id)
                .first::<Option<String>>(&mut conn)?;
            let vocabulary = for_system(
                &systems_dir,
                system_id.as_deref(),
                std::slice::from_ref(&wanted),
            );
            Ok::<_, diesel::result::Error>(vocabulary.get(&wanted).map(|kind| kind.binds))
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to read this world's system"))?
    };

    match binds {
        Some(Binds::Item) => {}
        // An unrecognised type has no declared binding in this world. Refused
        // rather than allowed: attaching content the active system cannot
        // describe is the one case where "leave it alone" is the safe answer.
        _ => {
            return Err(Error::new(format!(
                "Abilities of type \"{classification}\" do not attach to items in this world"
            )));
        }
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let snapshot = ability.name.clone();
    let grade = ability.grade;

    let inserted = tokio::task::spawn_blocking(move || {
        diesel::insert_into(world_item_abilities::table)
            .values(&NewItemAbility {
                item_id,
                ability_id: Some(ability_id),
                ability_name_snapshot: snapshot,
                created_by: user_id,
                updated_by: user_id,
            })
            .on_conflict((
                world_item_abilities::item_id,
                world_item_abilities::ability_id,
            ))
            .do_nothing()
            .returning(world_item_abilities::id)
            .get_result::<Uuid>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("This item already carries that ability"))?;

    Ok(GraphQLItemAbilityEntry {
        id: inserted,
        ability_id: Some(ability_id),
        ability_name: ability.name,
        classification: Some(ability.classification),
        grade,
    })
}

/// Every ability an item carries, for the item's own sheet (FR-020).
pub async fn item_abilities_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    item_id: Uuid,
) -> GraphQLResult<Vec<GraphQLItemAbilityEntry>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let world_id = tokio::task::spawn_blocking(move || {
        world_items::table
            .filter(world_items::id.eq(item_id))
            .select(world_items::world_id)
            .first::<Uuid>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Item not found"))?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        require_world_member(&mut conn, user_id, world_id)
            .map_err(|_| diesel::result::Error::NotFound)?;
        let _ = is_admin;

        let rows: Vec<(Uuid, Option<Uuid>, String, Option<String>, Option<i32>)> =
            world_item_abilities::table
                .left_join(
                    world_abilities::table.on(world_abilities::id
                        .nullable()
                        .eq(world_item_abilities::ability_id)),
                )
                .filter(world_item_abilities::item_id.eq(item_id))
                .select((
                    world_item_abilities::id,
                    world_item_abilities::ability_id,
                    world_item_abilities::ability_name_snapshot,
                    world_abilities::classification.nullable(),
                    world_abilities::grade.nullable(),
                ))
                .load(&mut conn)?;

        Ok::<_, diesel::result::Error>(
            rows.into_iter()
                .map(
                    |(id, ability_id, snapshot, classification, grade)| GraphQLItemAbilityEntry {
                        id,
                        ability_id,
                        ability_name: snapshot,
                        classification,
                        grade,
                    },
                )
                .collect(),
        )
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to read this item's abilities"))
}

use crate::state::AppState;

#[derive(Default)]
pub struct ItemAbilityMutation;

#[Object]
impl ItemAbilityMutation {
    /// Attach an ability to an item, if its type binds to items (FR-019).
    async fn attach_ability_to_item(
        &self,
        ctx: &Context<'_>,
        input: AttachAbilityToItemInput,
    ) -> GraphQLResult<GraphQLItemAbilityEntry> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        attach_ability_to_item_impl(state, auth_user.user_id, auth_user.is_admin, input).await
    }
}

#[derive(Default)]
pub struct ItemAbilityQuery;

#[Object]
impl ItemAbilityQuery {
    /// The abilities this item carries.
    async fn item_abilities(
        &self,
        ctx: &Context<'_>,
        item_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLItemAbilityEntry>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        item_abilities_impl(state, auth_user.user_id, auth_user.is_admin, item_id).await
    }
}

#[cfg(test)]
#[path = "mutations_item_abilities_tests.rs"]
mod tests;
