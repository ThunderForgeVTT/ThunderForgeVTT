//! Spec 025: an actor's known abilities — "this NPC knows Fireball"
//! (`actorAbilities`, `attachAbilityToActor`, `detachAbilityFromActor`).
//! See contracts/graphql-actor-abilities.md.
//!
//! **The load-bearing rule: permission is checked against the ACTOR, never the
//! ability** (FR-022). A user with Editor on an actor may attach any ability
//! in that world to them, even one they only have Viewer access to; conversely
//! Owner on an ability grants no right to attach it to an actor they cannot
//! edit. This is spec 013's rule for inventory, and it is what makes "the GM
//! equips an NPC with a spell the players can't read yet" work.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::actor_permissions::{is_dm_of_world, require_actor_permission};
use crate::graphql::types::{
    AbilityClassification, ActorPermissionLevel, GraphQLActorAbilityEntry,
};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{ActorAbilityEntry, NewActorAbilityEntry};
use crate::schema::{world_abilities, world_actor_abilities, world_actors};
use crate::state::AppState;

#[derive(InputObject, Debug, Clone)]
pub struct AttachAbilityToActorInput {
    pub actor_id: Uuid,
    pub ability_id: Uuid,
}

/// A joined row: the entry plus whatever survives of its ability.
type EntryRow = (ActorAbilityEntry, Option<String>, Option<bool>);

/// The name shown in place of a tombstoned ability for a non-DM caller.
pub const REDACTED_ABILITY_NAME: &str = "REDACTED";

fn to_graphql(row: EntryRow, caller_is_dm: bool) -> GraphQLActorAbilityEntry {
    let (entry, classification, gm_only) = row;
    let is_tombstone = entry.ability_id.is_none();
    GraphQLActorAbilityEntry {
        id: entry.id,
        actor_id: entry.actor_id,
        ability_id: entry.ability_id,
        // Fail closed: a tombstone carries no gm_only flag to check, so a
        // non-DM never sees the snapshotted name.
        ability_name: if is_tombstone && !caller_is_dm {
            REDACTED_ABILITY_NAME.to_string()
        } else {
            entry.ability_name_snapshot
        },
        classification: classification
            .as_deref()
            .and_then(AbilityClassification::from_db_str),
        gm_only: gm_only.unwrap_or(false),
    }
}

async fn actor_world_id(state: &AppState, actor_id: Uuid) -> GraphQLResult<Uuid> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_actors::table
            .filter(world_actors::id.eq(actor_id))
            .select(world_actors::world_id)
            .first::<Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load actor"))?
    .ok_or_else(|| Error::new("Actor not found"))
}

/// Testable core of `ActorAbilityQuery::actor_abilities` (FR-023).
///
/// Requires Viewer **on the actor**. GM-only abilities are filtered out for a
/// non-DM, silently — no placeholder, no count, no ordering gap, so a player
/// cannot infer that anything was withheld (FR-024b).
///
/// Tombstoned entries (ability deleted) keep their row so the actor's history
/// stays legible, but their **name is redacted for non-DMs**.
///
/// Once an ability row is gone there is no `gm_only` flag left to consult, so
/// there is no way to tell whether a tombstone used to be secret. Rather than
/// leak the name of a deleted GM-only ability, this fails closed: every
/// tombstone reads `REDACTED` to a non-DM. The cost is that a player also
/// stops seeing the name of an ordinary deleted ability — an acceptable trade,
/// since a deleted ability's name is of little use to a player and the
/// alternative leaks secrets.
pub async fn actor_abilities_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> GraphQLResult<Vec<GraphQLActorAbilityEntry>> {
    require_actor_permission(
        state,
        user_id,
        is_admin,
        actor_id,
        ActorPermissionLevel::Viewer,
    )
    .await?;

    let world_id = actor_world_id(state, actor_id).await?;
    let caller_is_dm = is_dm_of_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let rows = tokio::task::spawn_blocking(move || {
        world_actor_abilities::table
            .left_join(
                world_abilities::table
                    .on(world_actor_abilities::ability_id.eq(world_abilities::id.nullable())),
            )
            .filter(world_actor_abilities::actor_id.eq(actor_id))
            .order(world_actor_abilities::ability_name_snapshot.asc())
            .select((
                ActorAbilityEntry::as_select(),
                world_abilities::classification.nullable(),
                world_abilities::gm_only.nullable(),
            ))
            .load::<EntryRow>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load actor abilities"))?;

    Ok(rows
        .into_iter()
        // FR-024b: a live GM-only ability is omitted for a non-DM. A
        // tombstoned row (gm_only = None) is kept — see the doc comment.
        .filter(|(_, _, gm_only)| caller_is_dm || !gm_only.unwrap_or(false))
        .map(|row| to_graphql(row, caller_is_dm))
        .collect())
}

/// Testable core of `attach_ability_to_actor` (FR-021, FR-022).
///
/// Re-attaching an already-known ability is a **no-op returning the existing
/// entry**, not an error and not a duplicate row.
pub async fn attach_ability_to_actor_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: AttachAbilityToActorInput,
) -> GraphQLResult<GraphQLActorAbilityEntry> {
    // FR-022: the ACTOR's permission, deliberately not the ability's.
    require_actor_permission(
        state,
        user_id,
        is_admin,
        input.actor_id,
        ActorPermissionLevel::Editor,
    )
    .await?;

    let actor_world = actor_world_id(state, input.actor_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let ability_id = input.ability_id;
    let actor_id = input.actor_id;

    let ability = tokio::task::spawn_blocking(move || {
        world_abilities::table
            .filter(world_abilities::id.eq(ability_id))
            .select((
                world_abilities::world_id,
                world_abilities::name,
                world_abilities::classification,
                world_abilities::gm_only,
            ))
            .first::<(Uuid, String, String, bool)>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load ability"))?
    .ok_or_else(|| Error::new("Ability not found"))?;

    let (ability_world, ability_name, classification, gm_only) = ability;

    // Neither the FKs nor the UNIQUE constraint prevent a cross-world
    // reference, so it needs an explicit guard.
    if ability_world != actor_world {
        return Err(Error::new(
            "That ability belongs to a different world than this actor",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let name_for_insert = ability_name.clone();
    let entry = tokio::task::spawn_blocking(move || {
        // FR-021: re-attaching is a no-op. DO NOTHING returns no row, so fall
        // back to selecting the existing one.
        let inserted = diesel::insert_into(world_actor_abilities::table)
            .values(&NewActorAbilityEntry {
                actor_id,
                ability_id: Some(ability_id),
                ability_name_snapshot: name_for_insert,
            })
            .on_conflict((
                world_actor_abilities::actor_id,
                world_actor_abilities::ability_id,
            ))
            .do_nothing()
            .returning(ActorAbilityEntry::as_returning())
            .get_result::<ActorAbilityEntry>(&mut conn)
            .optional()?;

        match inserted {
            Some(row) => Ok(row),
            None => world_actor_abilities::table
                .filter(world_actor_abilities::actor_id.eq(actor_id))
                .filter(world_actor_abilities::ability_id.eq(ability_id))
                .select(ActorAbilityEntry::as_select())
                .first::<ActorAbilityEntry>(&mut conn),
        }
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to attach ability to actor"))?;

    // A freshly attached entry is never a tombstone, so redaction cannot
    // apply — pass `true` rather than re-deriving DM status.
    Ok(to_graphql((entry, Some(classification), Some(gm_only)), true))
}

/// Testable core of `detach_ability_from_actor` (FR-023).
///
/// Deletes only the entry row — the ability itself is untouched.
pub async fn detach_ability_from_actor_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    entry_id: Uuid,
) -> GraphQLResult<bool> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let actor_id = tokio::task::spawn_blocking(move || {
        world_actor_abilities::table
            .filter(world_actor_abilities::id.eq(entry_id))
            .select(world_actor_abilities::actor_id)
            .first::<Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load known-ability entry"))?
    .ok_or_else(|| Error::new("Known-ability entry not found"))?;

    require_actor_permission(
        state,
        user_id,
        is_admin,
        actor_id,
        ActorPermissionLevel::Editor,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::delete(
            world_actor_abilities::table.filter(world_actor_abilities::id.eq(entry_id)),
        )
        .execute(&mut conn)
        .map(|rows| rows > 0)
        .map_err(|e| format!("Failed to detach ability: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

#[derive(Default)]
pub struct ActorAbilityQuery;

#[async_graphql::Object]
impl ActorAbilityQuery {
    async fn actor_abilities(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLActorAbilityEntry>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        actor_abilities_impl(state, auth_user.user_id, auth_user.is_admin, actor_id).await
    }
}

#[derive(Default)]
pub struct ActorAbilityMutation;

#[async_graphql::Object]
impl ActorAbilityMutation {
    async fn attach_ability_to_actor(
        &self,
        ctx: &Context<'_>,
        input: AttachAbilityToActorInput,
    ) -> GraphQLResult<GraphQLActorAbilityEntry> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        attach_ability_to_actor_impl(state, auth_user.user_id, auth_user.is_admin, input).await
    }

    async fn detach_ability_from_actor(
        &self,
        ctx: &Context<'_>,
        entry_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        detach_ability_from_actor_impl(state, auth_user.user_id, auth_user.is_admin, entry_id)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_abilities::{create_ability_impl, delete_ability_impl, CreateAbilityInput};
    use crate::graphql::types::AbilityClassification;
    use crate::schema::{world_ability_permissions, world_actor_permissions};
    use crate::test_support::*;

    fn make_actor(
        conn: &mut diesel::PgConnection,
        world_id: Uuid,
        scene_id: Uuid,
        owner_id: Uuid,
    ) -> Uuid {
        let id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_actors::table)
            .values((
                world_actors::id.eq(id),
                world_actors::world_id.eq(world_id),
                world_actors::scene_id.eq(scene_id),
                world_actors::actor_type.eq("character"),
                world_actors::game_system_id.eq("generic"),
                world_actors::label.eq("Test Villain"),
                world_actors::created_by.eq(owner_id),
                world_actors::owned_by.eq(owner_id),
                world_actors::is_public.eq(false),
                world_actors::is_npc.eq(true),
                world_actors::created_at.eq(now),
                world_actors::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("insert test actor");
        id
    }

    fn grant_actor(conn: &mut diesel::PgConnection, actor_id: Uuid, user_id: Uuid, level: &str) {
        diesel::insert_into(world_actor_permissions::table)
            .values((
                world_actor_permissions::id.eq(Uuid::now_v7()),
                world_actor_permissions::actor_id.eq(actor_id),
                world_actor_permissions::user_id.eq(user_id),
                world_actor_permissions::level.eq(level),
            ))
            .execute(conn)
            .expect("grant actor permission");
    }

    fn ability_input(world_id: Uuid, name: &str) -> CreateAbilityInput {
        CreateAbilityInput {
            world_id,
            name: name.to_string(),
            description: None,
            classification: AbilityClassification::Spell,
            gm_only: None,
        }
    }

    /// FR-022, the load-bearing rule: permission follows the ACTOR, not the
    /// ability. Editor-on-actor + Viewer-on-ability succeeds; Owner-on-ability
    /// + Viewer-on-actor is rejected.
    #[tokio::test]
    async fn actor_ability_permission_follows_actor_not_ability() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let editor_id = insert_test_user(&mut conn);
        let viewer_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        insert_test_world_member(&mut conn, world_id, editor_id, "Player");
        insert_test_world_member(&mut conn, world_id, viewer_id, "Player");
        let actor_id = make_actor(&mut conn, world_id, scene_id, owner_id);
        drop(conn);

        let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Fireball"))
            .await
            .unwrap();

        let mut conn = state.db_pool.get().unwrap();
        // editor: Editor on the ACTOR, nothing on the ability (=> Viewer).
        grant_actor(&mut conn, actor_id, editor_id, "Editor");
        // viewer: Owner on the ABILITY, nothing on the actor (=> Viewer).
        diesel::insert_into(world_ability_permissions::table)
            .values((
                world_ability_permissions::id.eq(Uuid::now_v7()),
                world_ability_permissions::ability_id.eq(ability.id),
                world_ability_permissions::user_id.eq(viewer_id),
                world_ability_permissions::level.eq("Owner"),
            ))
            .execute(&mut conn)
            .unwrap();
        drop(conn);

        attach_ability_to_actor_impl(
            &state,
            editor_id,
            false,
            AttachAbilityToActorInput { actor_id, ability_id: ability.id },
        )
        .await
        .expect("Editor on the actor may attach an ability they only view");

        let err = attach_ability_to_actor_impl(
            &state,
            viewer_id,
            false,
            AttachAbilityToActorInput { actor_id, ability_id: ability.id },
        )
        .await
        .expect_err("Owner on the ability must NOT grant attach rights on the actor");
        assert!(err.message.contains("sufficient permission"));
    }

    /// FR-021: re-attaching is a no-op returning the existing entry.
    #[tokio::test]
    async fn attaching_same_ability_twice_is_a_noop() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = make_actor(&mut conn, world_id, scene_id, owner_id);
        drop(conn);

        let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Cleave"))
            .await
            .unwrap();
        let input = AttachAbilityToActorInput { actor_id, ability_id: ability.id };

        let first = attach_ability_to_actor_impl(&state, owner_id, false, input.clone())
            .await
            .unwrap();
        let second = attach_ability_to_actor_impl(&state, owner_id, false, input)
            .await
            .expect("re-attaching must be a no-op, not an error");

        assert_eq!(first.id, second.id, "the same entry must be returned");
        assert_eq!(
            actor_abilities_impl(&state, owner_id, false, actor_id).await.unwrap().len(),
            1,
            "no duplicate row"
        );
    }

    /// Neither the FKs nor the UNIQUE constraint prevent a cross-world
    /// reference — it needs an explicit guard.
    #[tokio::test]
    async fn attaching_cross_world_ability_is_rejected() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_a = insert_test_world(&mut conn, owner_id);
        let world_b = insert_test_world(&mut conn, owner_id);
        let scene_a = insert_test_scene(&mut conn, world_a, owner_id);
        let actor_id = make_actor(&mut conn, world_a, scene_a, owner_id);
        drop(conn);

        let foreign = create_ability_impl(&state, owner_id, false, ability_input(world_b, "Alien"))
            .await
            .unwrap();

        let err = attach_ability_to_actor_impl(
            &state,
            owner_id,
            false,
            AttachAbilityToActorInput { actor_id, ability_id: foreign.id },
        )
        .await
        .expect_err("an ability from another world must be rejected");
        assert!(err.message.contains("different world"));
    }

    /// FR-023: deleting an ability tombstones the entry instead of blocking.
    #[tokio::test]
    async fn deleting_an_ability_tombstones_actor_entries_instead_of_blocking() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = make_actor(&mut conn, world_id, scene_id, owner_id);
        drop(conn);

        let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Doomed"))
            .await
            .unwrap();
        attach_ability_to_actor_impl(
            &state,
            owner_id,
            false,
            AttachAbilityToActorInput { actor_id, ability_id: ability.id },
        )
        .await
        .unwrap();

        // The delete must succeed despite the actor knowing it.
        assert!(delete_ability_impl(&state, owner_id, false, ability.id)
            .await
            .expect("deletion must not be blocked by an actor knowing the ability"));

        let entries = actor_abilities_impl(&state, owner_id, false, actor_id).await.unwrap();
        assert_eq!(entries.len(), 1, "the entry survives as a tombstone");
        assert_eq!(entries[0].ability_id, None, "its reference is nulled");
        assert_eq!(
            entries[0].ability_name, "Doomed",
            "the DM still sees the name snapshot"
        );
        assert_eq!(entries[0].classification, None);
    }

    /// A tombstone carries no `gm_only` flag to consult, so a non-DM must not
    /// see its name at all — otherwise deleting a GM-only ability would leak
    /// the very name that was being hidden. Fails closed: every tombstone
    /// reads REDACTED to a player, secret or not.
    #[tokio::test]
    async fn tombstoned_ability_names_are_redacted_for_non_dms() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let player_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let actor_id = make_actor(&mut conn, world_id, scene_id, owner_id);
        grant_actor(&mut conn, actor_id, player_id, "Viewer");
        drop(conn);

        // A GM-only ability the player could never see while it existed...
        let mut secret_input = ability_input(world_id, "Soul Harvest");
        secret_input.gm_only = Some(true);
        let secret = create_ability_impl(&state, owner_id, false, secret_input)
            .await
            .unwrap();
        // ...and an ordinary one.
        let open = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Cleave"))
            .await
            .unwrap();

        for ability_id in [secret.id, open.id] {
            attach_ability_to_actor_impl(
                &state,
                owner_id,
                false,
                AttachAbilityToActorInput { actor_id, ability_id },
            )
            .await
            .unwrap();
            delete_ability_impl(&state, owner_id, false, ability_id)
                .await
                .unwrap();
        }

        let player_view = actor_abilities_impl(&state, player_id, false, actor_id).await.unwrap();
        assert_eq!(player_view.len(), 2, "tombstones stay listed");
        for entry in &player_view {
            assert_eq!(
                entry.ability_name, REDACTED_ABILITY_NAME,
                "a player must not see any tombstoned ability's name"
            );
        }
        assert!(
            !player_view.iter().any(|e| e.ability_name.contains("Soul Harvest")),
            "the deleted GM-only ability's name must not leak"
        );

        // The DM still sees the real names — redaction is for players only.
        let dm_view = actor_abilities_impl(&state, owner_id, false, actor_id).await.unwrap();
        let dm_names: Vec<&str> = dm_view.iter().map(|e| e.ability_name.as_str()).collect();
        assert!(dm_names.contains(&"Soul Harvest"));
        assert!(dm_names.contains(&"Cleave"));
    }

    /// FR-023/FR-024b: a non-DM's list silently omits GM-only abilities.
    #[tokio::test]
    async fn gm_only_abilities_are_omitted_from_a_non_dms_known_list() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let player_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let actor_id = make_actor(&mut conn, world_id, scene_id, owner_id);
        grant_actor(&mut conn, actor_id, player_id, "Viewer");
        drop(conn);

        let open = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Cleave"))
            .await
            .unwrap();
        let mut secret_input = ability_input(world_id, "Soul Harvest");
        secret_input.gm_only = Some(true);
        let secret = create_ability_impl(&state, owner_id, false, secret_input)
            .await
            .unwrap();

        for ability_id in [open.id, secret.id] {
            attach_ability_to_actor_impl(
                &state,
                owner_id,
                false,
                AttachAbilityToActorInput { actor_id, ability_id },
            )
            .await
            .unwrap();
        }

        let dm_view = actor_abilities_impl(&state, owner_id, false, actor_id).await.unwrap();
        assert_eq!(dm_view.len(), 2, "the DM sees both");

        let player_view = actor_abilities_impl(&state, player_id, false, actor_id).await.unwrap();
        assert_eq!(player_view.len(), 1, "the player must not see the GM-only ability");
        assert_eq!(player_view[0].ability_name, "Cleave");
        assert!(
            !player_view.iter().any(|e| e.ability_name == "Soul Harvest"),
            "no trace of the hidden ability may appear"
        );
    }

    /// US3 scenario 6: detaching removes the entry, not the ability.
    #[tokio::test]
    async fn detaching_does_not_delete_the_ability() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = make_actor(&mut conn, world_id, scene_id, owner_id);
        drop(conn);

        let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Keeper"))
            .await
            .unwrap();
        let entry = attach_ability_to_actor_impl(
            &state,
            owner_id,
            false,
            AttachAbilityToActorInput { actor_id, ability_id: ability.id },
        )
        .await
        .unwrap();

        assert!(detach_ability_from_actor_impl(&state, owner_id, false, entry.id)
            .await
            .unwrap());

        assert!(
            actor_abilities_impl(&state, owner_id, false, actor_id).await.unwrap().is_empty(),
            "the entry is gone"
        );
        // ...but the ability itself still exists in the world catalog.
        crate::graphql::queries::ability::ability_impl(&state, owner_id, false, ability.id)
            .await
            .expect("detaching must not delete the ability");
    }
}
