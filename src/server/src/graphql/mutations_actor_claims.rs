//! Spec 017: player onboarding — invite-to-actor selection. Actor
//! "available for claiming" flag, atomic claiming, player-created
//! characters, and GM un-claim. See
//! specs/017-invite-actor-selection/contracts/graphql-actor-claim.md.

use async_graphql::{Context, Error, Result as GraphQLResult};
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use uuid::Uuid;

use crate::auth::actor_permissions::require_actor_permission;
use crate::graphql::types::ActorPermissionLevel;
use crate::graphql::{
    GraphQLActorClaim, GraphQLWorldActor, GraphQLWorldMember, app_state, authenticated_user,
};
use crate::models::{ActorClaim, NewActorClaim, NewWorldActor, WorldActor, WorldMember};
use crate::schema::{users, world_actor_claims, world_actors, world_members, worlds};
use crate::state::AppState;

/// `conn.transaction`'s closure error type requires `From<diesel::result::Error>`
/// (the wrapper itself may fail to BEGIN/COMMIT) — mirrors
/// `mutations_actor_shares.rs`'s `CopyError` for the same reason.
#[derive(Debug)]
struct ClaimError(String);

impl From<diesel::result::Error> for ClaimError {
    fn from(e: diesel::result::Error) -> Self {
        ClaimError(e.to_string())
    }
}

impl From<String> for ClaimError {
    fn from(s: String) -> Self {
        ClaimError(s)
    }
}

fn actor_claim_to_graphql(claim: ActorClaim, claimed_by_user_id: Uuid) -> GraphQLActorClaim {
    GraphQLActorClaim {
        actor_id: claim.actor_id,
        world_member_id: claim.world_member_id,
        claimed_by_user_id,
        claimed_at: claim.claimed_at,
    }
}

/// Loads a `GraphQLWorldActor` by id — shared by `GraphQLActorClaim::actor`.
pub async fn load_actor_impl(state: &AppState, actor_id: Uuid) -> GraphQLResult<GraphQLWorldActor> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let actor = tokio::task::spawn_blocking(move || {
        world_actors::table
            .filter(world_actors::id.eq(actor_id))
            .select(WorldActor::as_select())
            .first::<WorldActor>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Actor not found"))?;

    Ok(GraphQLWorldActor::from(actor))
}

/// Who currently has `actor_id` claimed, if anyone — shared by
/// `GraphQLWorldActor::claimed_by` (FR-012).
pub async fn claimed_by_impl(
    state: &AppState,
    actor_id: Uuid,
) -> GraphQLResult<Option<GraphQLWorldMember>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let row = tokio::task::spawn_blocking(move || {
        world_actor_claims::table
            .inner_join(world_members::table)
            .inner_join(users::table.on(world_members::user_id.eq(users::id)))
            .filter(world_actor_claims::actor_id.eq(actor_id))
            .select((
                world_members::id,
                world_members::world_id,
                world_members::user_id,
                users::username,
            ))
            .first::<(Uuid, Uuid, Uuid, String)>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| Error::new(format!("Failed to look up claim: {e}")))?;

    Ok(
        row.map(|(id, world_id, user_id, username)| GraphQLWorldMember {
            id,
            world_id,
            user_id,
            username,
        }),
    )
}

/// Spec 023 (FR-004): the character `member_id` has claimed, if any — the
/// reverse of `claimed_by_impl` (this reads the same `world_actor_claims`
/// row, joined the other direction). `None` when the member hasn't
/// claimed a character.
pub async fn claimed_actor_impl(
    state: &AppState,
    member_id: Uuid,
) -> GraphQLResult<Option<GraphQLWorldActor>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let actor = tokio::task::spawn_blocking(move || {
        world_actor_claims::table
            .inner_join(world_actors::table)
            .filter(world_actor_claims::world_member_id.eq(member_id))
            .select(WorldActor::as_select())
            .first::<WorldActor>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| Error::new(format!("Failed to look up claimed actor: {e}")))?;

    Ok(actor.map(GraphQLWorldActor::from))
}

/// `myActorClaim(worldId)`: `None` for the GM/Owner role (FR-003) or a
/// non-GM member with no claim; otherwise the claimed `GraphQLActorClaim`.
pub async fn my_actor_claim_impl(
    state: &AppState,
    user_id: Uuid,
    world_id: Uuid,
) -> GraphQLResult<Option<GraphQLActorClaim>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        let member: Option<WorldMember> = world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(user_id))
            .select(WorldMember::as_select())
            .first::<WorldMember>(&mut conn)
            .optional()?;

        let Some(member) = member else {
            return Ok(None);
        };

        if thunderforge_authz::Role::from_stored(&member.role)
            .is_some_and(thunderforge_authz::Role::runs_the_world)
        {
            return Ok(None);
        }

        let claim: Option<ActorClaim> = world_actor_claims::table
            .filter(world_actor_claims::world_member_id.eq(member.id))
            .select(ActorClaim::as_select())
            .first::<ActorClaim>(&mut conn)
            .optional()?;

        Ok(claim.map(|c| actor_claim_to_graphql(c, member.user_id)))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e: diesel::result::Error| Error::new(format!("Failed to look up claim: {e}")))
}

/// `availableActors(worldId)`: PC-classified, flagged available, and
/// currently unclaimed actors in the world.
pub async fn available_actors_impl(
    state: &AppState,
    world_id: Uuid,
) -> GraphQLResult<Vec<WorldActor>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let rows = tokio::task::spawn_blocking(move || {
        world_actors::table
            .filter(world_actors::world_id.eq(world_id))
            .filter(world_actors::is_npc.eq(false))
            .filter(world_actors::available_for_claim.eq(true))
            .filter(diesel::dsl::not(diesel::dsl::exists(
                world_actor_claims::table.filter(world_actor_claims::actor_id.eq(world_actors::id)),
            )))
            .select(WorldActor::as_select())
            .load::<WorldActor>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load available actors"))?;

    crate::moderation::filter_visible(state, "world_actor", rows, |a| a.id).await
}

/// Looks up the caller's `world_members` row for `world_id`, and rejects
/// (a) non-members, (b) the GM/Owner role (this feature is non-GM-only),
/// and (c) a member who already holds a claim in this world (FR-014).
/// Shared by `claimActor`/`createAndClaimActor`.
fn require_no_existing_claim(
    conn: &mut PgConnection,
    world_id: Uuid,
    user_id: Uuid,
) -> Result<WorldMember, String> {
    let member = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .filter(world_members::user_id.eq(user_id))
        .select(WorldMember::as_select())
        .first::<WorldMember>(conn)
        .map_err(|_| "You are not a member of this world".to_string())?;

    if thunderforge_authz::Role::from_stored(&member.role)
        .is_some_and(thunderforge_authz::Role::runs_the_world)
    {
        return Err("The GM does not claim characters".to_string());
    }

    let already_claimed = world_actor_claims::table
        .filter(world_actor_claims::world_member_id.eq(member.id))
        .count()
        .get_result::<i64>(conn)
        .map_err(|e| format!("Failed to check existing claim: {e}"))?;

    if already_claimed > 0 {
        return Err("You have already claimed a character in this world".to_string());
    }

    Ok(member)
}

/// Testable core of `ActorClaimMutation::claim_actor`. Atomic: an
/// application-level availability check plus the table's `UNIQUE(actor_id)`
/// constraint as the concurrency backstop (research.md §4) — a lost race
/// surfaces as a specific "already claimed" error, never a silent
/// double-claim.
pub async fn claim_actor_impl(
    state: &AppState,
    user_id: Uuid,
    world_id: Uuid,
    actor_id: Uuid,
) -> GraphQLResult<GraphQLActorClaim> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let result = tokio::task::spawn_blocking(move || {
        conn.transaction(|conn| -> Result<ActorClaim, ClaimError> {
            let member = require_no_existing_claim(conn, world_id, user_id)?;

            let actor = world_actors::table
                .filter(world_actors::id.eq(actor_id))
                .filter(world_actors::world_id.eq(world_id))
                .select(WorldActor::as_select())
                .first::<WorldActor>(conn)
                .map_err(|_| "Actor not found in this world".to_string())?;

            if actor.is_npc || !actor.available_for_claim {
                return Err("This character is not available to claim"
                    .to_string()
                    .into());
            }

            let new_claim = NewActorClaim {
                actor_id: actor.id,
                world_member_id: member.id,
            };

            diesel::insert_into(world_actor_claims::table)
                .values(&new_claim)
                .returning(ActorClaim::as_returning())
                .get_result::<ActorClaim>(conn)
                .map_err(|e| {
                    ClaimError(match e {
                        DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _) => {
                            "This character was just claimed by someone else".to_string()
                        }
                        other => format!("Failed to claim character: {other}"),
                    })
                })
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e: ClaimError| Error::new(e.0))?;

    Ok(actor_claim_to_graphql(result, user_id))
}

/// Testable core of `ActorClaimMutation::create_and_claim_actor`. Re-checks
/// `allow_player_created_actors` server-side regardless of client UI state
/// (FR-008/FR-009) — no race is possible since the actor doesn't exist for
/// anyone else to contend over until this transaction commits.
pub async fn create_and_claim_actor_impl(
    state: &AppState,
    user_id: Uuid,
    world_id: Uuid,
    name: String,
    description: Option<String>,
) -> GraphQLResult<GraphQLActorClaim> {
    if name.trim().is_empty() {
        return Err(Error::new("Character name must not be empty"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let result = tokio::task::spawn_blocking(move || {
        conn.transaction(|conn| -> Result<ActorClaim, ClaimError> {
            let member = require_no_existing_claim(conn, world_id, user_id)?;

            let allow: bool = worlds::table
                .filter(worlds::id.eq(world_id))
                .select(worlds::allow_player_created_actors)
                .first::<bool>(conn)
                .map_err(|_| "World not found".to_string())?;

            if !allow {
                return Err("This world's GM has not enabled player-created characters"
                    .to_string()
                    .into());
            }

            let scene_id = crate::schema::scenes::table
                .filter(crate::schema::scenes::world_id.eq(world_id))
                .order(crate::schema::scenes::created_at.asc())
                .select(crate::schema::scenes::scene_id)
                .first::<Uuid>(conn)
                .map_err(|_| "World has no scenes to assign the new character to".to_string())?;

            let new_actor = NewWorldActor {
                world_id,
                scene_id,
                actor_type: "character".to_string(),
                game_system_id: Some("generic".to_string()),
                label: name,
                created_by: user_id,
                owned_by: user_id,
                is_public: false,
                is_npc: false,
                description,
            };

            let created = diesel::insert_into(world_actors::table)
                .values(&new_actor)
                .returning(WorldActor::as_returning())
                .get_result::<WorldActor>(conn)
                .map_err(|e| format!("Failed to create character: {e}"))?;

            // available_for_claim defaults to false at insert time (the
            // migration's column default); flip it true so this new,
            // already-claimed character is consistent with data-model.md's
            // rule that a claimed actor's flag reflects reality even
            // though it's excluded from `availableActors` while claimed.
            diesel::update(world_actors::table.filter(world_actors::id.eq(created.id)))
                .set(world_actors::available_for_claim.eq(true))
                .execute(conn)
                .map_err(|e| format!("Failed to flag new character available: {e}"))?;

            let new_claim = NewActorClaim {
                actor_id: created.id,
                world_member_id: member.id,
            };

            diesel::insert_into(world_actor_claims::table)
                .values(&new_claim)
                .returning(ActorClaim::as_returning())
                .get_result::<ActorClaim>(conn)
                .map_err(|e| ClaimError(format!("Failed to claim newly created character: {e}")))
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e: ClaimError| Error::new(e.0))?;

    Ok(actor_claim_to_graphql(result, user_id))
}

/// Testable core of `ActorClaimMutation::set_actor_availability`. Requires
/// Owner-level Actor permission — reuses spec 010's existing check
/// verbatim, no new authority (research.md §6).
pub async fn set_actor_availability_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
    available: bool,
) -> GraphQLResult<GraphQLWorldActor> {
    require_actor_permission(
        state,
        user_id,
        is_admin,
        actor_id,
        ActorPermissionLevel::Owner,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let updated = tokio::task::spawn_blocking(move || {
        let actor = world_actors::table
            .filter(world_actors::id.eq(actor_id))
            .select(WorldActor::as_select())
            .first::<WorldActor>(&mut conn)
            .map_err(|_| "Actor not found".to_string())?;

        if available && actor.is_npc {
            return Err("Only player characters can be marked available for claiming".to_string());
        }

        diesel::update(world_actors::table.filter(world_actors::id.eq(actor_id)))
            .set(world_actors::available_for_claim.eq(available))
            .returning(WorldActor::as_returning())
            .get_result::<WorldActor>(&mut conn)
            .map_err(|e| format!("Failed to update availability: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLWorldActor::from(updated))
}

/// Testable core of `ActorClaimMutation::unclaim_actor`. Same Owner-level
/// check as `set_actor_availability_impl` (GM authority, Clarifications
/// Q3). Does NOT touch `available_for_claim` — an unclaimed, still-flagged
/// actor becomes visible again automatically (data-model.md).
pub async fn unclaim_actor_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> GraphQLResult<GraphQLWorldActor> {
    require_actor_permission(
        state,
        user_id,
        is_admin,
        actor_id,
        ActorPermissionLevel::Owner,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let actor = tokio::task::spawn_blocking(move || {
        diesel::delete(world_actor_claims::table.filter(world_actor_claims::actor_id.eq(actor_id)))
            .execute(&mut conn)
            .map_err(|e| format!("Failed to unclaim character: {e}"))?;

        world_actors::table
            .filter(world_actors::id.eq(actor_id))
            .select(WorldActor::as_select())
            .first::<WorldActor>(&mut conn)
            .map_err(|e| format!("Actor not found after unclaim: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLWorldActor::from(actor))
}

#[derive(Default)]
pub struct ActorClaimMutation;

#[async_graphql::Object]
impl ActorClaimMutation {
    async fn claim_actor(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        actor_id: Uuid,
    ) -> GraphQLResult<GraphQLActorClaim> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        claim_actor_impl(state, auth_user.user_id, world_id, actor_id).await
    }

    async fn create_and_claim_actor(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        name: String,
        description: Option<String>,
    ) -> GraphQLResult<GraphQLActorClaim> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        create_and_claim_actor_impl(state, auth_user.user_id, world_id, name, description).await
    }

    async fn set_actor_availability(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
        available: bool,
    ) -> GraphQLResult<GraphQLWorldActor> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        set_actor_availability_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            actor_id,
            available,
        )
        .await
    }

    async fn unclaim_actor(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
    ) -> GraphQLResult<GraphQLWorldActor> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        unclaim_actor_impl(state, auth_user.user_id, auth_user.is_admin, actor_id).await
    }
}

#[derive(Default)]
pub struct ActorClaimQuery;

#[async_graphql::Object]
impl ActorClaimQuery {
    async fn my_actor_claim(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Option<GraphQLActorClaim>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        my_actor_claim_impl(state, auth_user.user_id, world_id).await
    }

    async fn available_actors(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLWorldActor>> {
        let state = app_state(ctx)?;
        let _auth_user = authenticated_user(ctx)?;
        let actors = available_actors_impl(state, world_id).await?;
        Ok(actors.into_iter().map(GraphQLWorldActor::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
        test_app_state,
    };

    fn mark_available(conn: &mut PgConnection, actor_id: Uuid, available: bool) {
        diesel::update(world_actors::table.filter(world_actors::id.eq(actor_id)))
            .set(world_actors::available_for_claim.eq(available))
            .execute(conn)
            .expect("failed to mark actor availability");
    }

    fn insert_test_pc(
        conn: &mut PgConnection,
        world_id: Uuid,
        scene_id: Uuid,
        owner_id: Uuid,
        label: &str,
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
                world_actors::label.eq(label),
                world_actors::created_by.eq(owner_id),
                world_actors::owned_by.eq(owner_id),
                world_actors::is_public.eq(false),
                world_actors::is_npc.eq(false),
                world_actors::created_at.eq(now),
                world_actors::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("failed to insert test PC actor");
        id
    }

    fn set_allow_player_created(conn: &mut PgConnection, world_id: Uuid, allow: bool) {
        diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::allow_player_created_actors.eq(allow))
            .execute(conn)
            .expect("failed to set allow_player_created_actors");
    }

    #[tokio::test]
    async fn non_member_cannot_claim() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
        mark_available(&mut conn, actor_id, true);
        let outsider_id = insert_test_user(&mut conn);
        drop(conn);

        let result = claim_actor_impl(&state, outsider_id, world_id, actor_id).await;
        assert!(
            result.is_err(),
            "a non-member must not be able to claim a character"
        );
    }

    #[tokio::test]
    async fn gm_never_gated_myactorclaim_always_none() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let claim = my_actor_claim_impl(&state, owner_id, world_id)
            .await
            .expect("query should succeed for the owner");
        assert!(
            claim.is_none(),
            "the GM/Owner must never be shown a claim gate"
        );
    }

    #[tokio::test]
    async fn claiming_unavailable_actor_errors() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        // available_for_claim defaults to false — never marked available.
        let result = claim_actor_impl(&state, player_id, world_id, actor_id).await;
        assert!(result.is_err(), "an unflagged actor must not be claimable");
    }

    #[tokio::test]
    async fn claim_succeeds_and_actor_disappears_from_available_list() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
        mark_available(&mut conn, actor_id, true);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let before = available_actors_impl(&state, world_id).await.unwrap();
        assert_eq!(before.len(), 1);

        let claim = claim_actor_impl(&state, player_id, world_id, actor_id)
            .await
            .expect("claim should succeed");
        assert_eq!(claim.actor_id, actor_id);

        let after = available_actors_impl(&state, world_id).await.unwrap();
        assert!(
            after.is_empty(),
            "a claimed actor must disappear from the available list"
        );

        let my_claim = my_actor_claim_impl(&state, player_id, world_id)
            .await
            .unwrap();
        assert!(
            my_claim.is_some(),
            "the claiming player should now see their claim"
        );
    }

    // ===== Spec 023: claimed_actor_impl (the Players section's roster join) =====

    #[tokio::test]
    async fn claimed_actor_impl_returns_none_before_a_claim_and_the_actor_after() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
        mark_available(&mut conn, actor_id, true);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        // Need the member's own `world_members.id` (not `user_id`) — fetch
        // it the same way `claimed_by_impl`'s reverse resolver does.
        let mut conn = state.db_pool.get().unwrap();
        let member_id: Uuid = world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(player_id))
            .select(world_members::id)
            .first(&mut conn)
            .unwrap();
        drop(conn);

        let before = claimed_actor_impl(&state, member_id).await.unwrap();
        assert!(
            before.is_none(),
            "no claim yet — must be None, not an error"
        );

        let claim = claim_actor_impl(&state, player_id, world_id, actor_id)
            .await
            .expect("claim should succeed");
        assert_eq!(claim.world_member_id, member_id);

        let after = claimed_actor_impl(&state, member_id).await.unwrap();
        assert_eq!(
            after.map(|a| a.id),
            Some(actor_id),
            "after claiming, claimed_actor_impl must return that same actor"
        );
    }

    #[tokio::test]
    async fn member_with_existing_claim_cannot_claim_second_actor() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let first_actor = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
        let second_actor = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Borin");
        mark_available(&mut conn, first_actor, true);
        mark_available(&mut conn, second_actor, true);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        claim_actor_impl(&state, player_id, world_id, first_actor)
            .await
            .expect("first claim should succeed");

        let result = claim_actor_impl(&state, player_id, world_id, second_actor).await;
        assert!(
            result.is_err(),
            "a member with an existing claim must not claim a second character"
        );
    }

    #[tokio::test]
    async fn create_and_claim_rejected_when_setting_off() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let result = create_and_claim_actor_impl(
            &state,
            player_id,
            world_id,
            "Homebrew Hero".to_string(),
            None,
        )
        .await;
        assert!(
            result.is_err(),
            "creation must be rejected when the world setting is off"
        );
    }

    #[tokio::test]
    async fn create_and_claim_succeeds_when_setting_on() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        set_allow_player_created(&mut conn, world_id, true);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let claim = create_and_claim_actor_impl(
            &state,
            player_id,
            world_id,
            "Homebrew Hero".to_string(),
            None,
        )
        .await
        .expect("creation should succeed when the setting is on");

        let my_claim = my_actor_claim_impl(&state, player_id, world_id)
            .await
            .unwrap();
        assert_eq!(my_claim.unwrap().actor_id, claim.actor_id);
    }

    #[tokio::test]
    async fn set_availability_rejects_non_owner() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let result = set_actor_availability_impl(&state, player_id, false, actor_id, true).await;
        assert!(
            result.is_err(),
            "a non-Owner caller must not be able to set availability"
        );
    }

    #[tokio::test]
    async fn set_availability_rejects_npc() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let npc_id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_actors::table)
            .values((
                world_actors::id.eq(npc_id),
                world_actors::world_id.eq(world_id),
                world_actors::scene_id.eq(scene_id),
                world_actors::actor_type.eq("npc"),
                world_actors::game_system_id.eq("generic"),
                world_actors::label.eq("Goblin"),
                world_actors::created_by.eq(owner_id),
                world_actors::owned_by.eq(owner_id),
                world_actors::is_public.eq(false),
                world_actors::is_npc.eq(true),
                world_actors::created_at.eq(now),
                world_actors::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .unwrap();
        drop(conn);

        let result = set_actor_availability_impl(&state, owner_id, false, npc_id, true).await;
        assert!(
            result.is_err(),
            "an NPC-classified actor must not be markable as available"
        );
    }

    #[tokio::test]
    async fn unclaim_makes_actor_available_again_without_reflagging() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
        mark_available(&mut conn, actor_id, true);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        claim_actor_impl(&state, player_id, world_id, actor_id)
            .await
            .expect("claim should succeed");
        assert!(
            available_actors_impl(&state, world_id)
                .await
                .unwrap()
                .is_empty()
        );

        unclaim_actor_impl(&state, owner_id, false, actor_id)
            .await
            .expect("the DM should be able to unclaim");

        let available = available_actors_impl(&state, world_id).await.unwrap();
        assert_eq!(
            available.len(),
            1,
            "the actor should reappear as available without re-flagging"
        );

        let previous_claimant = my_actor_claim_impl(&state, player_id, world_id)
            .await
            .unwrap();
        assert!(
            previous_claimant.is_none(),
            "the previous claimant should return to the no-character-selected state"
        );

        // The un-claimed player's world_members row must remain untouched
        // (they stay a full world member, per FR-013).
        let still_member: bool = diesel::select(diesel::dsl::exists(
            world_members::table
                .filter(world_members::world_id.eq(world_id))
                .filter(world_members::user_id.eq(player_id)),
        ))
        .get_result::<bool>(&mut state.db_pool.get().unwrap())
        .unwrap();
        assert!(
            still_member,
            "un-claiming must not remove the player from the world"
        );
    }

    #[tokio::test]
    async fn concurrent_claims_exactly_one_succeeds() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
        mark_available(&mut conn, actor_id, true);
        let player_a = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_a, "Player");
        let player_b = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_b, "Player");
        drop(conn);

        let (result_a, result_b) = tokio::join!(
            claim_actor_impl(&state, player_a, world_id, actor_id),
            claim_actor_impl(&state, player_b, world_id, actor_id),
        );

        let successes = [result_a.is_ok(), result_b.is_ok()]
            .iter()
            .filter(|ok| **ok)
            .count();
        assert_eq!(
            successes, 1,
            "exactly one of two concurrent claims must succeed (FR-006/SC-003)"
        );

        // Sanity: the unique constraint is genuinely load-bearing, not
        // just the app-level pre-check — force a raw duplicate insert
        // past the app-level guard to confirm the DB itself rejects it.
        let mut conn = state.db_pool.get().unwrap();
        let member_id: Uuid = world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(player_a))
            .select(world_members::id)
            .first(&mut conn)
            .unwrap();
        let dup = diesel::sql_query(
            "INSERT INTO world_actor_claims (actor_id, world_member_id) VALUES ($1, $2)",
        )
        .bind::<diesel::sql_types::Uuid, _>(actor_id)
        .bind::<diesel::sql_types::Uuid, _>(member_id)
        .execute(&mut conn);
        assert!(
            dup.is_err(),
            "the UNIQUE(actor_id) constraint must reject a duplicate claim row"
        );
    }
}
