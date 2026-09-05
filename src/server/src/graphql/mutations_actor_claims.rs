//! Spec 017: player onboarding — invite-to-actor selection. Actor
//! "available for claiming" flag, atomic claiming, player-created
//! characters, and GM un-claim. See
//! specs/017-invite-actor-selection/contracts/graphql-actor-claim.md.

use async_graphql::{Context, Error, ErrorExtensions, Result as GraphQLResult};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use uuid::Uuid;

use crate::auth::actor_permissions::require_actor_permission;
use crate::graphql::types::ActorPermissionLevel;
use crate::graphql::{
    GraphQLActorClaim, GraphQLWorldActor, GraphQLWorldMember, app_state, authenticated_user,
};
use crate::models::{ActorClaim, NewActorClaim, NewWorldActor, WorldActor, WorldMember};
use crate::schema::{users, world_actor_claims, world_actors, world_members, worlds};
use crate::state::AppState;

/// The extension code the loser of a contested character receives.
///
/// Spec 031 FR-034. Three surfaces write this relation — the players
/// section, the actor page, and a player's own selection screen — and all
/// three have to be able to tell "somebody else got there first" from "the
/// request failed". A generic refusal would have a Game Master re-trying a
/// binding that is working exactly as designed.
pub const ALREADY_CLAIMED: &str = "ALREADY_CLAIMED";

/// The extension code an un-claim receives when the claim it was looking at
/// is no longer the one on the row.
///
/// The actor page shows who holds a character and offers to release them.
/// Between that read and the click, the players section may have bound
/// somebody else. Without this the release would silently erase a binding
/// its operator never saw — the one outcome FR-034 is written to prevent.
pub const CLAIM_CHANGED: &str = "CLAIM_CHANGED";

/// `conn.transaction`'s closure error type requires `From<diesel::result::Error>`
/// (the wrapper itself may fail to BEGIN/COMMIT) — mirrors
/// `mutations_actor_shares.rs`'s `CopyError` for the same reason.
///
/// An enum rather than the plain string it used to be: a lost race has to
/// keep its identity all the way out to `extensions.code`, and a message
/// compared by text would break the moment somebody improved the wording.
#[derive(Debug)]
enum ClaimError {
    AlreadyClaimed,
    ClaimChanged,
    Message(String),
}

impl From<diesel::result::Error> for ClaimError {
    fn from(e: diesel::result::Error) -> Self {
        ClaimError::Message(e.to_string())
    }
}

impl From<String> for ClaimError {
    fn from(s: String) -> Self {
        ClaimError::Message(s)
    }
}

impl From<ClaimError> for Error {
    fn from(e: ClaimError) -> Self {
        match e {
            ClaimError::AlreadyClaimed => already_claimed(),
            ClaimError::ClaimChanged => {
                Error::new("That character's player has changed since this page was loaded")
                    .extend_with(|_, ext| ext.set("code", CLAIM_CHANGED))
            }
            ClaimError::Message(message) => Error::new(message),
        }
    }
}

fn already_claimed() -> Error {
    Error::new("That character is already played by someone else")
        .extend_with(|_, ext| ext.set("code", ALREADY_CLAIMED))
}

/// Bind `member_id` to `actor_id`, if nobody has taken either side yet.
///
/// The single arbiter of this relation. A conditional insert, and whether a
/// row came back is the whole answer: `world_actor_claims` is unique on
/// `actor_id` *and* on `world_member_id`, so `ON CONFLICT DO NOTHING`
/// covers both halves of FR-034 — a character claimed twice, and a player
/// bound to two characters — in one statement that Postgres serialises on
/// the index. Every writer goes through here rather than issuing its own
/// insert, which is what makes "all three agree" a property of the code
/// rather than a promise.
///
/// A read-then-insert was the obvious alternative and is wrong for the
/// usual reason: the gap between the read and the write is exactly where
/// the second caller lands.
fn bind_claim(
    conn: &mut PgConnection,
    actor_id: Uuid,
    member_id: Uuid,
) -> Result<Option<ActorClaim>, DieselError> {
    diesel::insert_into(world_actor_claims::table)
        .values(&NewActorClaim {
            actor_id,
            world_member_id: member_id,
        })
        .on_conflict_do_nothing()
        .returning(ActorClaim::as_returning())
        .get_result::<ActorClaim>(conn)
        .optional()
}

/// Which of the two unique constraints refused a `bind_claim`.
///
/// Both surface as zero rows, and they are different news: the character is
/// spoken for, or this player already has one. Read after the fact rather
/// than inspecting the constraint name, because Diesel reports the name as
/// free text and the two paths need different messages, not different
/// parsing.
fn explain_failed_bind(conn: &mut PgConnection, actor_id: Uuid) -> Result<ClaimError, DieselError> {
    let taken = world_actor_claims::table
        .filter(world_actor_claims::actor_id.eq(actor_id))
        .count()
        .get_result::<i64>(conn)?
        > 0;

    Ok(if taken {
        ClaimError::AlreadyClaimed
    } else {
        ClaimError::Message("That player already plays another character".to_string())
    })
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
/// application-level availability check plus `bind_claim` as the arbiter
/// (research.md §4) — a lost race surfaces as `ALREADY_CLAIMED`, never a
/// silent double-claim, and it is the same arbiter the players section's
/// `set_player_character_binding_impl` goes through (spec 031 FR-034).
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

            match bind_claim(conn, actor.id, member.id)? {
                Some(claim) => Ok(claim),
                None => Err(explain_failed_bind(conn, actor.id)?),
            }
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::from)?;

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

            // Through the same arbiter as every other writer, even though
            // this character was created a line ago and nobody else can be
            // contending for it. One code path for the relation is the
            // point of FR-034; an "it cannot race here" shortcut is how
            // the three writers drifted apart in the first place.
            match bind_claim(conn, created.id, member.id)? {
                Some(claim) => Ok(claim),
                None => Err(explain_failed_bind(conn, created.id)?),
            }
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::from)?;

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
///
/// `expected_world_member_id` is spec 031 FR-034's half of the agreement
/// between the three writers. When the caller says which claim it is
/// releasing, the delete is conditional on that still being the claim on
/// the row, and a mismatch is refused as `CLAIM_CHANGED` rather than
/// quietly destroying a binding made in between. `None` keeps the original
/// unconditional behaviour for callers that hold no such expectation.
pub async fn unclaim_actor_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
    expected_world_member_id: Option<Uuid>,
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
        let mut delete = diesel::delete(
            world_actor_claims::table.filter(world_actor_claims::actor_id.eq(actor_id)),
        )
        .into_boxed();

        if let Some(member_id) = expected_world_member_id {
            delete = delete.filter(world_actor_claims::world_member_id.eq(member_id));
        }

        let released = delete
            .execute(&mut conn)
            .map_err(|e| ClaimError::Message(format!("Failed to unclaim character: {e}")))?;

        // Zero rows with a stated expectation means the row moved on: the
        // character is either free already or now played by somebody else.
        // Either way the operator is looking at a stale screen, and the
        // honest answer is to say so and let them re-read it.
        if released == 0 && expected_world_member_id.is_some() {
            return Err(ClaimError::ClaimChanged);
        }

        world_actors::table
            .filter(world_actors::id.eq(actor_id))
            .select(WorldActor::as_select())
            .first::<WorldActor>(&mut conn)
            .map_err(|e| ClaimError::Message(format!("Actor not found after unclaim: {e}")))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::from)?;

    Ok(GraphQLWorldActor::from(actor))
}

/// Testable core of `ActorClaimMutation::set_player_character_binding` —
/// spec 031 FR-034, the players section's own writer of this relation.
///
/// # Who may
///
/// A Game Master, over somebody else's binding. That is not the authority
/// `claim_actor` uses (a player, over their own) and not the one the actor
/// page uses (Owner-level permission on one actor); it is authority over
/// the *world*, so it asks the same question `mutations_invites.rs` asks
/// before a role change: `is_dm_of_world`. Constitution Principle III —
/// the players section's picker is chrome, and this is where the answer
/// actually lives.
///
/// # Re-binding, not stacking
///
/// Setting a binding for a player who already has one releases the old
/// claim first, inside the same transaction. The alternative — refusing
/// until the GM un-binds by hand — is two steps for what a GM experiences
/// as one correction, and it leaves a window in which the player has
/// nobody. `None` for `actor_id` is that release on its own.
///
/// A character somebody else already plays is refused rather than
/// re-pointed, because taking a character away from a player is a decision
/// the GM should make deliberately on that player's card, not a side
/// effect of a picker.
pub async fn set_player_character_binding_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    world_member_id: Uuid,
    actor_id: Option<Uuid>,
) -> GraphQLResult<Option<GraphQLWorldActor>> {
    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(
            Error::new("Only Owners and GMs can set a player's character")
                .extend_with(|_, ext| ext.set("code", "FORBIDDEN")),
        );
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let bound = tokio::task::spawn_blocking(move || {
        conn.transaction(|conn| -> Result<Option<WorldActor>, ClaimError> {
            let member: WorldMember = world_members::table
                .filter(world_members::id.eq(world_member_id))
                .filter(world_members::world_id.eq(world_id))
                .select(WorldMember::as_select())
                .first::<WorldMember>(conn)
                .map_err(|_| "That player is not a member of this world".to_string())?;

            diesel::delete(
                world_actor_claims::table.filter(world_actor_claims::world_member_id.eq(member.id)),
            )
            .execute(conn)?;

            let Some(actor_id) = actor_id else {
                return Ok(None);
            };

            let actor = world_actors::table
                .filter(world_actors::id.eq(actor_id))
                .filter(world_actors::world_id.eq(world_id))
                .select(WorldActor::as_select())
                .first::<WorldActor>(conn)
                .map_err(|_| "Character not found in this world".to_string())?;

            if actor.is_npc {
                return Err("An NPC cannot be given to a player".to_string().into());
            }

            // The arbiter. A refusal here rolls back the release above, so
            // a lost race leaves the player on the character they had
            // rather than on nobody.
            if bind_claim(conn, actor.id, member.id)?.is_none() {
                return Err(explain_failed_bind(conn, actor.id)?);
            }

            // `available_for_claim` is deliberately left alone. It means
            // "offered on the selection screen", and a GM handing out a
            // character has not offered it to the room — flipping it true
            // here would put the character on that screen the moment this
            // binding was lifted.
            Ok(Some(actor))
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::from)?;

    Ok(bound.map(GraphQLWorldActor::from))
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

    /// Free a claimed character. `expected_world_member_id` names the claim
    /// the caller was looking at, so a release cannot land on one it never
    /// saw (spec 031 FR-034).
    async fn unclaim_actor(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
        expected_world_member_id: Option<Uuid>,
    ) -> GraphQLResult<GraphQLWorldActor> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        unclaim_actor_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            actor_id,
            expected_world_member_id,
        )
        .await
    }

    /// Set (or clear, with a null `actor_id`) which character a player is
    /// playing — the players section's writer of the claim relation.
    async fn set_player_character_binding(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        world_member_id: Uuid,
        actor_id: Option<Uuid>,
    ) -> GraphQLResult<Option<GraphQLWorldActor>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        set_player_character_binding_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            world_member_id,
            actor_id,
        )
        .await
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
#[path = "mutations_actor_claims_tests.rs"]
mod tests;
