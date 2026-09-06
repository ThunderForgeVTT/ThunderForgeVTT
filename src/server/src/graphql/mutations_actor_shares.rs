//! Spec 010: actor sharing and cross-world copy (`createActorShareLink`,
//! `revokeActorShareLink`, `sharedActor`, `copySharedActorToWorld`). See
//! contracts/actor-share.md.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::actor_permissions::effective_actor_permission;
use crate::auth::world_membership::is_dm_of_world;
use crate::graphql::anonymous::caller_id;
use crate::graphql::share_codes::generate_link_code;
use crate::graphql::share_rate_limit as rate_limit;
use crate::graphql::types::{ActorPermissionLevel, GraphQLActorShareLink, SharedActorPreview};
use crate::graphql::{GraphQLActorSystemData, GraphQLWorldActor, app_state, authenticated_user};
use crate::models::{ActorShare, ActorSystemData, NewActorShare, NewWorldActor, WorldActor};
use crate::schema::{scenes, world_actor_shares, world_actor_system_data, world_actors};
use crate::state::AppState;

#[derive(InputObject, Debug, Clone)]
pub struct CopySharedActorInput {
    pub share_code: String,
    pub destination_world_id: Uuid,
}

/// Local error wrapper so the Diesel transaction closure in
/// `copy_shared_actor_to_world_impl` can mix diesel errors (via `?`) with
/// our own descriptive `String` messages — `diesel::Connection::transaction`
/// requires the closure's error type to implement `From<diesel::result::Error>`,
/// which we can't implement directly on `String` (orphan rule).
struct CopyError(String);

impl From<diesel::result::Error> for CopyError {
    fn from(e: diesel::result::Error) -> Self {
        CopyError(e.to_string())
    }
}

impl From<String> for CopyError {
    fn from(s: String) -> Self {
        CopyError(s)
    }
}

/// The one sentence every failed lookup in this module produces.
///
/// ADR-071: an unknown code, a revoked share, a deleted actor and a moderated
/// one must be indistinguishable to an outsider, because distinguishing them is
/// a probe — and that matters more now the caller need not have an account. A
/// constant rather than four string literals so they cannot drift apart later,
/// which is exactly how this kind of leak is usually introduced.
pub const UNAVAILABLE: &str = "This share link is no longer available";

fn load_active_share(
    conn: &mut diesel::PgConnection,
    share_code: &str,
) -> Result<ActorShare, String> {
    world_actor_shares::table
        .filter(world_actor_shares::share_code.eq(share_code))
        .filter(world_actor_shares::revoked.eq(false))
        .select(ActorShare::as_select())
        .first::<ActorShare>(conn)
        .map_err(|_| UNAVAILABLE.to_string())
}

/// Testable core of `sharedActor` (research.md §9).
///
/// **Unauthenticated** — ADR-071. Do not add `authenticated_user(ctx)?` to the
/// resolver that calls this; the session requirement was removed deliberately,
/// on the same terms ADR-070 set for `sharedCollection`. There is no
/// world-membership check either, which is the point of a share link.
///
/// `caller` is used for one thing: rate limiting, before the lookup. An
/// unguessable code is unguessable only while the number of guesses is bounded,
/// and once no account is needed, nothing else bounds them.
///
/// Blocked entirely for a moderated actor, so a share can never become a
/// moderation bypass.
///
/// Returns a world-identity-scrubbed projection.
pub async fn shared_actor_impl(
    state: &AppState,
    caller: &str,
    share_code: String,
) -> GraphQLResult<SharedActorPreview> {
    // ADR-071 (and FR-009c's reasoning): before the lookup, never after.
    if !rate_limit::allow_request(caller) {
        return Err(Error::new(rate_limit::rate_limited_message()));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let (actor, system_data) = tokio::task::spawn_blocking(move || {
        let share = load_active_share(&mut conn, &share_code)?;

        let actor = world_actors::table
            .filter(world_actors::id.eq(share.actor_id))
            .select(WorldActor::as_select())
            .first::<WorldActor>(&mut conn)
            .map_err(|_| UNAVAILABLE.to_string())?;

        let system_data = world_actor_system_data::table
            .filter(world_actor_system_data::actor_id.eq(actor.id))
            .select(ActorSystemData::as_select())
            .first::<ActorSystemData>(&mut conn)
            .optional()
            .map_err(|e| format!("Failed to load actor system data: {e}"))?;

        Ok::<_, String>((actor, system_data))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    // Spec 015: a share link must not become a moderation bypass — a
    // disabled actor's real content must never leak through this path.
    if crate::moderation::effective_status(state, "world_actor", actor.id)
        .await?
        .is_some()
    {
        return Err(Error::new(UNAVAILABLE));
    }

    Ok(SharedActorPreview {
        label: actor.label,
        actor_type: actor.actor_type,
        is_npc: actor.is_npc,
        game_system_id: actor.game_system_id,
        system_data: system_data.map(GraphQLActorSystemData::from),
    })
}

/// Testable core of `ActorShareMutation::create_actor_share_link`.
/// Requires effective `Owner` on the actor, including the DM's implicit
/// access (FR-023).
/// Testable core of `actorShareLink` — the active share for a actor the
/// caller owns, or null.
///
/// # Why it exists
///
/// ADR-071's second half. The revoke mutation shipped without a read path, so
/// revoking only worked inside the browser session that minted the link: the
/// code was shown once, and closing the tab removed the owner's ability to
/// recall it permanently. Spec 026 recorded that defect against all three
/// singleton shares and held it under FR-009e; collections answered it with
/// `collectionShareLink` and this is the same answer.
///
/// It matters more now the read is anonymous, not less: a link that reaches the
/// public and cannot be recalled by its owner is exactly what ADR-049's
/// ownership model exists to prevent.
///
/// # Why this is not the enumeration FR-020 forbids
///
/// It is scoped to one actor the caller already has Owner-level authority
/// over — the same authority needed to mint the link in the first place. It
/// reaches nothing by world, by user, or in aggregate, so nothing becomes
/// discoverable that was not already.
pub async fn actor_share_link_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> GraphQLResult<Option<ActorShare>> {
    // The authority to see a link is the authority to have made one.
    let level = effective_actor_permission(state, user_id, is_admin, actor_id).await?;
    if level.rank() < ActorPermissionLevel::Owner.rank() {
        return Err(Error::new(
            "Only an Owner-level member may see this actor's share link",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_actor_shares::table
            .filter(world_actor_shares::actor_id.eq(actor_id))
            .filter(world_actor_shares::revoked.eq(false))
            .order(world_actor_shares::created_at.desc())
            .select(ActorShare::as_select())
            .first::<ActorShare>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| Error::new(format!("Failed to load the share link: {e}")))
}

pub async fn create_actor_share_link_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> GraphQLResult<ActorShare> {
    let level = effective_actor_permission(state, user_id, is_admin, actor_id).await?;
    if level.rank() < ActorPermissionLevel::Owner.rank() {
        return Err(Error::new(
            "Only an Owner-level member may share this actor",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        let new_share = NewActorShare {
            id: Uuid::now_v7(),
            actor_id,
            share_code: generate_link_code(),
            created_by: user_id,
        };

        diesel::insert_into(world_actor_shares::table)
            .values(&new_share)
            .returning(ActorShare::as_returning())
            .get_result::<ActorShare>(&mut conn)
            .map_err(|e| format!("Failed to create share link: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `ActorShareMutation::revoke_actor_share_link`. Allowed
/// for the link's own creator OR the DM of the actor's world (FR-029).
pub async fn revoke_actor_share_link_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    share_id: Uuid,
) -> GraphQLResult<bool> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let share = tokio::task::spawn_blocking(move || {
        world_actor_shares::table
            .filter(world_actor_shares::id.eq(share_id))
            .select(ActorShare::as_select())
            .first::<ActorShare>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load share link"))?
    .ok_or_else(|| Error::new("Share link not found"))?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let actor_id = share.actor_id;
    let world_id = tokio::task::spawn_blocking(move || {
        world_actors::table
            .filter(world_actors::id.eq(actor_id))
            .select(world_actors::world_id)
            .first::<Uuid>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load actor"))?;

    let is_creator = share.created_by == user_id;
    let is_dm = is_dm_of_world(state, user_id, is_admin, world_id).await?;
    if !is_creator && !is_dm {
        return Err(Error::new(
            "Only the link's creator or the world's DM may revoke it",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::update(world_actor_shares::table.filter(world_actor_shares::id.eq(share_id)))
            .set((
                world_actor_shares::revoked.eq(true),
                world_actor_shares::updated_at.eq(Utc::now().naive_utc()),
            ))
            .execute(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to revoke share link"))?;

    Ok(true)
}

/// Testable core of `ActorShareMutation::copy_shared_actor_to_world`.
/// Re-verifies both the share link's validity and the caller's DM-level
/// access on the destination world server-side — never trusts a prior
/// `myDmWorlds` read (FR-025/026/027/030).
pub async fn copy_shared_actor_to_world_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: CopySharedActorInput,
) -> GraphQLResult<WorldActor> {
    let destination_world_id = input.destination_world_id;

    if !is_dm_of_world(state, user_id, is_admin, destination_world_id).await? {
        return Err(Error::new(
            "You must hold DM-level access on the destination world to copy an actor into it",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let share_code = input.share_code.clone();
    tokio::task::spawn_blocking(move || {
        conn.transaction(|conn| {
            let share = load_active_share(conn, &share_code)?;

            let source = world_actors::table
                .filter(world_actors::id.eq(share.actor_id))
                .select(WorldActor::as_select())
                .first::<WorldActor>(conn)
                .map_err(|_| UNAVAILABLE.to_string())?;

            let destination_scene_id = scenes::table
                .filter(scenes::world_id.eq(destination_world_id))
                .order(scenes::created_at.asc())
                .select(scenes::scene_id)
                .first::<Uuid>(conn)
                .map_err(|_| "Destination world has no scenes".to_string())?;

            let new_actor_row = NewWorldActor {
                world_id: destination_world_id,
                scene_id: destination_scene_id,
                actor_type: source.actor_type.clone(),
                game_system_id: source.game_system_id.clone(),
                label: source.label.clone(),
                created_by: user_id,
                owned_by: user_id,
                is_public: false,
                is_npc: source.is_npc,
                description: source.description.clone(),
            };

            let created = diesel::insert_into(world_actors::table)
                .values(&new_actor_row)
                .returning(WorldActor::as_returning())
                .get_result::<WorldActor>(conn)
                .map_err(|e| format!("Failed to create copied actor: {e}"))?;

            if let Some(system_data) = world_actor_system_data::table
                .filter(world_actor_system_data::actor_id.eq(source.id))
                .select(ActorSystemData::as_select())
                .first::<ActorSystemData>(conn)
                .optional()
                .map_err(|e| format!("Failed to load source actor system data: {e}"))?
            {
                diesel::insert_into(world_actor_system_data::table)
                    .values((
                        world_actor_system_data::id.eq(Uuid::now_v7()),
                        world_actor_system_data::actor_id.eq(created.id),
                        world_actor_system_data::game_system_id
                            .eq(system_data.game_system_id.clone()),
                        world_actor_system_data::ability_data.eq(system_data.ability_data.clone()),
                        world_actor_system_data::resource_data
                            .eq(system_data.resource_data.clone()),
                        world_actor_system_data::proficiency_data
                            .eq(system_data.proficiency_data.clone()),
                        world_actor_system_data::trait_data.eq(system_data.trait_data.clone()),
                        world_actor_system_data::spell_data.eq(system_data.spell_data.clone()),
                        world_actor_system_data::created_by.eq(user_id),
                        world_actor_system_data::updated_by.eq(user_id),
                    ))
                    .execute(conn)
                    .map_err(|e| format!("Failed to clone actor system data: {e}"))?;
            }

            Ok::<_, CopyError>(created)
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e: CopyError| Error::new(e.0))
}

#[derive(Default)]
pub struct ActorShareQuery;

#[async_graphql::Object]
impl ActorShareQuery {
    /// ADR-071: the active share link for a actor the caller owns, or null.
    /// **Authenticated**, and scoped to one actor the caller already has
    /// authority over — see `actor_share_link_impl` for why this is not
    /// enumeration.
    async fn actor_share_link(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
    ) -> GraphQLResult<Option<GraphQLActorShareLink>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        Ok(
            actor_share_link_impl(state, auth_user.user_id, auth_user.is_admin, actor_id)
                .await?
                .map(Into::into),
        )
    }

    /// **Deliberately unauthenticated** — ADR-071. Do not add
    /// `authenticated_user(ctx)?` here; it was removed on purpose, and all four
    /// share reads now agree. The caller is identified only to rate-limit them.
    async fn shared_actor(
        &self,
        ctx: &Context<'_>,
        share_code: String,
    ) -> GraphQLResult<SharedActorPreview> {
        let state = app_state(ctx)?;
        shared_actor_impl(state, &caller_id(ctx), share_code).await
    }
}

#[derive(Default)]
pub struct ActorShareMutation;

#[async_graphql::Object]
impl ActorShareMutation {
    async fn create_actor_share_link(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
    ) -> GraphQLResult<GraphQLActorShareLink> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        create_actor_share_link_impl(state, auth_user.user_id, auth_user.is_admin, actor_id)
            .await
            .map(GraphQLActorShareLink::from)
    }

    async fn revoke_actor_share_link(
        &self,
        ctx: &Context<'_>,
        share_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        revoke_actor_share_link_impl(state, auth_user.user_id, auth_user.is_admin, share_id).await
    }

    async fn copy_shared_actor_to_world(
        &self,
        ctx: &Context<'_>,
        input: CopySharedActorInput,
    ) -> GraphQLResult<GraphQLWorldActor> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        copy_shared_actor_to_world_impl(state, auth_user.user_id, auth_user.is_admin, input)
            .await
            .map(GraphQLWorldActor::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_actors::{CreateActorInput, create_actor_impl};
    use crate::test_support::{
        insert_test_scene, insert_test_user, insert_test_world, test_app_state,
    };

    /// A distinct caller per test, so the shared rate limiter's window cannot
    /// leak between them: they run concurrently in one process, and a limiter
    /// keyed on a constant would make passing depend on test order.
    fn a_caller() -> String {
        format!("test-{}", Uuid::new_v4())
    }

    /// FR-023: only an Owner-level member (including the DM's implicit
    /// access) may generate a share link.
    #[tokio::test]
    async fn create_share_link_requires_owner_level() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        let outsider_id = insert_test_user(&mut conn);
        drop(conn);

        let actor = create_actor_impl(
            &state,
            owner_id,
            false,
            CreateActorInput {
                world_id,
                label: "Bo Jangles".to_string(),
                is_npc: true,
                actor_type: None,
                game_system_id: None,
                description: None,
            },
        )
        .await
        .expect("DM should create actor");

        // A user with no relationship to this world at all (not even a
        // membership row) has no permission on the actor — default Viewer.
        let denied = create_actor_share_link_impl(&state, outsider_id, false, actor.id).await;
        assert!(
            denied.is_err(),
            "a non-Owner-level caller must not be able to share the actor"
        );

        let link = create_actor_share_link_impl(&state, owner_id, false, actor.id)
            .await
            .expect("the DM (implicit Owner) should be able to share the actor");
        assert!(!link.revoked);
        assert_eq!(link.actor_id, actor.id);
    }

    /// FR-024: `sharedActor` rejects a revoked code and never leaks the
    /// source world/scene/owner identity.
    #[tokio::test]
    async fn shared_actor_rejects_revoked_and_scrubs_identity() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let actor = create_actor_impl(
            &state,
            owner_id,
            false,
            CreateActorInput {
                world_id,
                label: "Bo Jangles".to_string(),
                is_npc: true,
                actor_type: Some("npc".to_string()),
                game_system_id: None,
                description: None,
            },
        )
        .await
        .expect("DM should create actor");

        let link = create_actor_share_link_impl(&state, owner_id, false, actor.id)
            .await
            .expect("DM should be able to share the actor");

        let preview = shared_actor_impl(&state, &a_caller(), link.share_code.clone())
            .await
            .expect("a valid share code should resolve");
        assert_eq!(preview.label, "Bo Jangles");

        revoke_actor_share_link_impl(&state, owner_id, false, link.id)
            .await
            .expect("DM should be able to revoke");

        let after_revoke = shared_actor_impl(&state, &a_caller(), link.share_code).await;
        assert!(
            after_revoke.is_err(),
            "a revoked share code must no longer resolve"
        );

        let missing = shared_actor_impl(&state, &a_caller(), "DOES-NOT-EXIST".to_string()).await;
        assert!(missing.is_err(), "an unknown share code must not resolve");
    }

    /// FR-026/027/030: a copy is a fully independent actor with cloned
    /// system data and an empty ownership block, and the destination DM
    /// requirement is re-checked server-side.
    #[tokio::test]
    async fn copy_produces_independent_actor_and_rechecks_destination_access() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let source_owner_id = insert_test_user(&mut conn);
        let source_world_id = insert_test_world(&mut conn, source_owner_id);
        insert_test_scene(&mut conn, source_world_id, source_owner_id);

        let dest_owner_id = insert_test_user(&mut conn);
        let dest_world_id = insert_test_world(&mut conn, dest_owner_id);
        insert_test_scene(&mut conn, dest_world_id, dest_owner_id);

        let uninvolved_id = insert_test_user(&mut conn);
        drop(conn);

        let source_actor = create_actor_impl(
            &state,
            source_owner_id,
            false,
            CreateActorInput {
                world_id: source_world_id,
                label: "Bo Jangles".to_string(),
                is_npc: true,
                actor_type: None,
                game_system_id: None,
                description: None,
            },
        )
        .await
        .expect("source DM should create actor");

        let link = create_actor_share_link_impl(&state, source_owner_id, false, source_actor.id)
            .await
            .expect("source DM should be able to share the actor");

        // A user with no DM-level access anywhere near the destination
        // world must be rejected, even with a valid share code.
        let denied = copy_shared_actor_to_world_impl(
            &state,
            uninvolved_id,
            false,
            CopySharedActorInput {
                share_code: link.share_code.clone(),
                destination_world_id: dest_world_id,
            },
        )
        .await;
        assert!(
            denied.is_err(),
            "a caller without DM access on the destination must be rejected"
        );

        let copy = copy_shared_actor_to_world_impl(
            &state,
            dest_owner_id,
            false,
            CopySharedActorInput {
                share_code: link.share_code,
                destination_world_id: dest_world_id,
            },
        )
        .await
        .expect("destination DM should be able to copy the shared actor");

        assert_ne!(
            copy.id, source_actor.id,
            "the copy must have a new identity"
        );
        assert_eq!(copy.world_id, dest_world_id);
        assert_eq!(copy.label, "Bo Jangles");

        let copy_permissions = crate::graphql::mutations_actor_permissions::actor_permissions_impl(
            &state,
            dest_owner_id,
            false,
            copy.id,
        )
        .await
        .expect("destination DM should be able to view the copy's ownership block");
        assert!(
            copy_permissions.is_empty(),
            "a freshly copied actor must start with an empty ownership block (FR-030)"
        );
    }

    /// ADR-071: with no account required, the refusal must not distinguish an
    /// unknown code from a revoked share. Distinguishing them is a probe, and
    /// the probe is now free.
    #[tokio::test]
    async fn a_revoked_share_is_indistinguishable_from_a_code_that_never_existed() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let actor = create_actor_impl(
            &state,
            owner_id,
            false,
            CreateActorInput {
                world_id,
                label: "Bo Jangles".to_string(),
                is_npc: true,
                actor_type: Some("npc".to_string()),
                game_system_id: None,
                description: None,
            },
        )
        .await
        .expect("DM should create actor");

        let link = create_actor_share_link_impl(&state, owner_id, false, actor.id)
            .await
            .expect("the owner may share");

        revoke_actor_share_link_impl(&state, owner_id, false, link.id)
            .await
            .expect("the owner may revoke");

        let revoked = shared_actor_impl(&state, &a_caller(), link.share_code)
            .await
            .expect_err("a revoked code must not resolve");
        let unknown = shared_actor_impl(&state, &a_caller(), "NOTAREALCODEATALL0".to_string())
            .await
            .expect_err("an unknown code must not resolve");

        assert_eq!(
            revoked.message, unknown.message,
            "the two refusals must be one sentence, or the difference is a probe"
        );
        assert_eq!(revoked.message, UNAVAILABLE);
    }

    /// ADR-071: the read resolves with no session at all. `shared_actor_impl`
    /// takes a caller only to rate-limit it, and is reached by a resolver that
    /// never calls `authenticated_user` — this asserts the core behaves that
    /// way rather than asserting the absence of a line of code.
    #[tokio::test]
    async fn the_read_needs_no_account() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let actor = create_actor_impl(
            &state,
            owner_id,
            false,
            CreateActorInput {
                world_id,
                label: "Bo Jangles".to_string(),
                is_npc: true,
                actor_type: Some("npc".to_string()),
                game_system_id: None,
                description: None,
            },
        )
        .await
        .expect("DM should create actor");

        let link = create_actor_share_link_impl(&state, owner_id, false, actor.id)
            .await
            .expect("the owner may share");

        let preview = shared_actor_impl(&state, "an-anonymous-visitor-actor", link.share_code)
            .await
            .expect("a valid code must resolve for a caller with no account");
        assert_eq!(preview.label, "Bo Jangles");
    }

    /// ADR-071: an unguessable code is unguessable only while the guesses are
    /// bounded, and the account requirement that used to bound them is gone.
    #[tokio::test]
    async fn the_anonymous_read_is_rate_limited() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let actor = create_actor_impl(
            &state,
            owner_id,
            false,
            CreateActorInput {
                world_id,
                label: "Bo Jangles".to_string(),
                is_npc: true,
                actor_type: Some("npc".to_string()),
                game_system_id: None,
                description: None,
            },
        )
        .await
        .expect("DM should create actor");

        let link = create_actor_share_link_impl(&state, owner_id, false, actor.id)
            .await
            .expect("the owner may share");

        let caller = a_caller();
        let mut refused = None;
        for _ in 0..200 {
            if let Err(e) = shared_actor_impl(&state, &caller, link.share_code.clone()).await {
                refused = Some(e);
                break;
            }
        }

        let error = refused.expect("a caller must eventually be rate limited");
        assert!(
            error.message.contains("Too many requests"),
            "got: {}",
            error.message
        );
        assert_ne!(
            error.message, UNAVAILABLE,
            "being throttled must not read as the code being invalid"
        );
    }

    /// ADR-071's second half: revoking must not depend on still having the page
    /// that minted the link. Before this read path, closing the tab lost the
    /// code permanently.
    #[tokio::test]
    async fn the_owner_can_recover_the_share_code_after_closing_the_page() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let actor = create_actor_impl(
            &state,
            owner_id,
            false,
            CreateActorInput {
                world_id,
                label: "Bo Jangles".to_string(),
                is_npc: true,
                actor_type: Some("npc".to_string()),
                game_system_id: None,
                description: None,
            },
        )
        .await
        .expect("DM should create actor");

        let link = create_actor_share_link_impl(&state, owner_id, false, actor.id)
            .await
            .expect("the owner may share");

        let recovered = actor_share_link_impl(&state, owner_id, false, actor.id)
            .await
            .expect("the owner may read back their own share link")
            .expect("an active share must be found");
        assert_eq!(recovered.share_code, link.share_code);
        assert_eq!(recovered.id, link.id);

        revoke_actor_share_link_impl(&state, owner_id, false, link.id)
            .await
            .expect("the owner may revoke");

        let after = actor_share_link_impl(&state, owner_id, false, actor.id)
            .await
            .expect("reading back after revocation is not an error");
        assert!(
            after.is_none(),
            "a revoked share is not an active share link"
        );
    }
}
