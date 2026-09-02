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

    /// The member ids currently claiming `actor_id`. Spec 031 tests assert on
    /// this rather than on what a mutation returned, because "exactly one
    /// player got the character" is a statement about the table — the same
    /// reason `mutations_pickup.rs` counts inventory rows.
    fn claimants_of(conn: &mut PgConnection, actor_id: Uuid) -> Vec<Uuid> {
        world_actor_claims::table
            .filter(world_actor_claims::actor_id.eq(actor_id))
            .select(world_actor_claims::world_member_id)
            .load::<Uuid>(conn)
            .expect("failed to read claims")
    }

    /// The actor ids `member_id` is bound to. More than one is the other
    /// half of FR-034 and would be just as much a bug as two claimants.
    fn characters_of(conn: &mut PgConnection, member_id: Uuid) -> Vec<Uuid> {
        world_actor_claims::table
            .filter(world_actor_claims::world_member_id.eq(member_id))
            .select(world_actor_claims::actor_id)
            .load::<Uuid>(conn)
            .expect("failed to read claims")
    }

    fn member_id_of(conn: &mut PgConnection, world_id: Uuid, user_id: Uuid) -> Uuid {
        world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(user_id))
            .select(world_members::id)
            .first::<Uuid>(conn)
            .expect("failed to read world member")
    }

    fn error_code(error: &Error) -> String {
        error
            .extensions
            .as_ref()
            .and_then(|ext| ext.get("code"))
            .map(|v| format!("{v:?}"))
            .unwrap_or_default()
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

        unclaim_actor_impl(&state, owner_id, false, actor_id, None)
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

    /// FR-034: a GM binds a player from the players section, and the row
    /// that results is the same relation every other surface reads.
    #[tokio::test]
    async fn gm_binds_a_player_to_a_character() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let member_id = member_id_of(&mut conn, world_id, player_id);
        drop(conn);

        set_player_character_binding_impl(
            &state,
            owner_id,
            false,
            world_id,
            member_id,
            Some(actor_id),
        )
        .await
        .expect("the world's owner may set a player's character");

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(claimants_of(&mut conn, actor_id), vec![member_id]);

        // The binding is visible to the player's own surfaces too — one
        // relation, not a parallel one that only the GM screen knows about.
        let claim = my_actor_claim_impl(&state, player_id, world_id)
            .await
            .unwrap()
            .expect("the bound player must see the character as theirs");
        assert_eq!(claim.actor_id, actor_id);
    }

    /// A GM correcting a binding replaces it. Two rows for one player would
    /// be the "player bound to two characters" FR-034 forbids.
    #[tokio::test]
    async fn rebinding_a_player_replaces_the_previous_character() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let first = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
        let second = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Bran");
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let member_id = member_id_of(&mut conn, world_id, player_id);
        drop(conn);

        for actor_id in [first, second] {
            set_player_character_binding_impl(
                &state,
                owner_id,
                false,
                world_id,
                member_id,
                Some(actor_id),
            )
            .await
            .expect("a GM may re-bind a player");
        }

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(characters_of(&mut conn, member_id), vec![second]);
        assert!(
            claimants_of(&mut conn, first).is_empty(),
            "the character the player was moved off must be free again"
        );
    }

    /// Clearing a binding leaves the player a member with no character.
    #[tokio::test]
    async fn clearing_a_binding_leaves_the_player_without_a_character() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let member_id = member_id_of(&mut conn, world_id, player_id);
        drop(conn);

        set_player_character_binding_impl(
            &state,
            owner_id,
            false,
            world_id,
            member_id,
            Some(actor_id),
        )
        .await
        .unwrap();
        let cleared =
            set_player_character_binding_impl(&state, owner_id, false, world_id, member_id, None)
                .await
                .expect("a GM may clear a binding");
        assert!(cleared.is_none());

        let mut conn = state.db_pool.get().unwrap();
        assert!(characters_of(&mut conn, member_id).is_empty());
        assert!(claimants_of(&mut conn, actor_id).is_empty());
    }

    /// A character somebody else plays is refused with the code the client
    /// keys on, and the existing binding is untouched.
    #[tokio::test]
    async fn binding_a_character_another_player_plays_is_refused() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
        let held_by = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, held_by, "Player");
        let holder_member = member_id_of(&mut conn, world_id, held_by);
        let latecomer = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, latecomer, "Player");
        let latecomer_member = member_id_of(&mut conn, world_id, latecomer);
        drop(conn);

        set_player_character_binding_impl(
            &state,
            owner_id,
            false,
            world_id,
            holder_member,
            Some(actor_id),
        )
        .await
        .unwrap();

        let refusal = set_player_character_binding_impl(
            &state,
            owner_id,
            false,
            world_id,
            latecomer_member,
            Some(actor_id),
        )
        .await
        .expect_err("a character already played may not be handed to a second player");
        assert!(
            error_code(&refusal).contains(ALREADY_CLAIMED),
            "the refusal must be distinguishable from a malfunction; got {refusal:?}"
        );

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            claimants_of(&mut conn, actor_id),
            vec![holder_member],
            "a refused binding must leave the standing one exactly as it was"
        );
    }

    /// FR-034 / Constitution III: the picker is chrome. A player calling the
    /// mutation directly for somebody else's binding is refused server-side.
    #[tokio::test]
    async fn a_player_may_not_set_another_players_binding() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
        let meddler = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, meddler, "Player");
        let victim = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, victim, "Player");
        let victim_member = member_id_of(&mut conn, world_id, victim);
        drop(conn);

        set_player_character_binding_impl(
            &state,
            meddler,
            false,
            world_id,
            victim_member,
            Some(actor_id),
        )
        .await
        .expect_err("a Player may not bind characters for other players");

        let mut conn = state.db_pool.get().unwrap();
        assert!(
            claimants_of(&mut conn, actor_id).is_empty(),
            "a refused binding must write nothing"
        );
    }

    /// The T067 case itself: the players section and a player's own claim
    /// screen going for the same character at the same moment. Exactly one
    /// row exists afterwards, and whoever lost is told which thing happened.
    #[tokio::test]
    async fn a_gm_binding_and_a_player_claim_cannot_both_win() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
        mark_available(&mut conn, actor_id, true);

        // Two different players, so a double-write shows up as two rows
        // rather than as one row written twice.
        let self_claimer = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, self_claimer, "Player");
        let bound_player = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, bound_player, "Player");
        let bound_member = member_id_of(&mut conn, world_id, bound_player);
        drop(conn);

        let binding = {
            let state = state.clone();
            tokio::spawn(async move {
                set_player_character_binding_impl(
                    &state,
                    owner_id,
                    false,
                    world_id,
                    bound_member,
                    Some(actor_id),
                )
                .await
                .map(|_| ())
            })
        };
        let self_claim = {
            let state = state.clone();
            tokio::spawn(async move {
                claim_actor_impl(&state, self_claimer, world_id, actor_id)
                    .await
                    .map(|_| ())
            })
        };

        let mut winners = 0;
        for attempt in [binding, self_claim] {
            match attempt.await.expect("claim task must not panic") {
                Ok(()) => winners += 1,
                Err(e) => assert!(
                    error_code(&e).contains(ALREADY_CLAIMED),
                    "the loser of a contested character must be told exactly \
                     that; got {e:?}"
                ),
            }
        }

        assert_eq!(winners, 1, "two writers, one character, one winner");

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            claimants_of(&mut conn, actor_id).len(),
            1,
            "a contested character must end up claimed exactly once"
        );
    }

    /// The actor page's release, aimed at a claim that has since moved.
    /// Without the conditional delete this test's binding would vanish.
    #[tokio::test]
    async fn unclaiming_a_stale_claim_changes_nothing() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
        let first_player = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, first_player, "Player");
        let first_member = member_id_of(&mut conn, world_id, first_player);
        let second_player = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, second_player, "Player");
        let second_member = member_id_of(&mut conn, world_id, second_player);
        drop(conn);

        // What the actor page rendered.
        set_player_character_binding_impl(
            &state,
            owner_id,
            false,
            world_id,
            first_member,
            Some(actor_id),
        )
        .await
        .unwrap();

        // What the players section did while that page sat open.
        set_player_character_binding_impl(&state, owner_id, false, world_id, first_member, None)
            .await
            .unwrap();
        set_player_character_binding_impl(
            &state,
            owner_id,
            false,
            world_id,
            second_member,
            Some(actor_id),
        )
        .await
        .unwrap();

        let refusal = unclaim_actor_impl(&state, owner_id, false, actor_id, Some(first_member))
            .await
            .expect_err("releasing a claim that has moved on must be refused");
        assert!(
            error_code(&refusal).contains(CLAIM_CHANGED),
            "a stale release is a changed claim, not a malfunction; got {refusal:?}"
        );

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            claimants_of(&mut conn, actor_id),
            vec![second_member],
            "a stale release must not erase the binding it never saw"
        );

        // The release the page would issue after re-reading does work.
        drop(conn);
        unclaim_actor_impl(&state, owner_id, false, actor_id, Some(second_member))
            .await
            .expect("releasing the claim that is actually there must succeed");
        let mut conn = state.db_pool.get().unwrap();
        assert!(claimants_of(&mut conn, actor_id).is_empty());
    }
}
