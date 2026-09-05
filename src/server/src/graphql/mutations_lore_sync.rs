//! Spec 034: establishing, acknowledging and removing a world's repository
//! connection.
//!
//! Four mutations, all owner-level (FR-002), and **none of them writes to a
//! world's lore**. That is the property that makes the first delivery safe by
//! construction rather than by care, and it is visible here as the absence of
//! any statement touching a lore table — the one exception being the test that
//! proves removal leaves those tables alone.
//!
//! # Authority is re-checked, never captured
//!
//! FR-003. Every mutation asks [`require_world_owner`] on the way in, against
//! the caller making *this* call. Nothing stores who created a connection as
//! though that settled anything: ownership of a world changes, and a
//! connection established by a former owner must stop being manageable by
//! them the moment they stop owning the world. The same rule applies per
//! synchronisation run, which is the scheduler's half of the requirement.
//!
//! # The grant boundary
//!
//! [`begin_grant`] and [`complete_grant`] are the two calls this module makes
//! into the host adapter, and they are the seam FR-004a asks to be pointed at.
//! Their implementations belong to **T014, `src/server/src/repo_host.rs`** —
//! the only file in `src/server` permitted to name a host. Until it exists
//! they answer with a plain refusal rather than a panic: an unfinished
//! integration must present as "this instance cannot do that yet", which is
//! the same thing an operator who has registered nothing sees, and never as a
//! crashed request.

use async_graphql::Enum;
use async_graphql::{Context, Error, ErrorExtensions, Object, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::graphql::queries::lore_sync::{
    GraphQLLoreRepositoryConnection, LoreSyncState, load_connection, require_world_owner,
};
use crate::graphql::{app_state, authenticated_user};
use crate::models::LoreRepositoryConnection;
use crate::schema::lore_repository_connections;
use crate::state::AppState;

/// Where the branch defaults, when the caller expresses no preference.
const DEFAULT_BRANCH: &str = "main";
/// Where a world's files go inside the repository, when the caller expresses
/// no preference. A subdirectory rather than the root, because FR-032
/// requires a first synchronisation into a repository with existing files to
/// leave them alone, and a default that writes at the root makes a collision
/// with the user's own `README.md` the *expected* first experience.
const DEFAULT_DIRECTORY: &str = "lore";

#[path = "mutations_lore_sync_grant.rs"]
pub mod grant;
pub use grant::*;
use grant::{begin_grant, complete_grant, normalize_branch, normalize_directory};

/// Testable core of `beginLoreRepositoryConnection` (the repo's `_impl`
/// convention, so tests need no GraphQL `Context`).
pub async fn begin_lore_repository_connection_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<ConnectionGrantHandoff> {
    require_world_owner(state, user_id, is_admin, world_id).await?;

    // FR-001, checked before the user is sent anywhere. The database enforces
    // it too, but discovering a duplicate *after* someone has installed an
    // application on a repository would mean asking them to undo something at
    // the host to recover from our own oversight.
    if load_connection(state, world_id).await?.is_some() {
        return Err(already_connected());
    }

    begin_grant(state, world_id, user_id).await
}

fn already_connected() -> Error {
    Error::new(
        "This world is already connected to a repository. Remove that connection before making another.",
    )
    .extend_with(|_, ext| ext.set("code", "ALREADY_CONNECTED"))
}

fn directory_claimed() -> Error {
    Error::new(
        "Another world is already synchronising into that directory of that repository. \
         Choose a different directory, or a different repository.",
    )
    .extend_with(|_, ext| ext.set("code", "REPOSITORY_DIRECTORY_CLAIMED"))
}

/// Testable core of `completeLoreRepositoryConnection`.
///
/// Creates nothing when any of its refusals fires. The two uniqueness rules
/// are checked here *and* enforced by the schema; the check here exists to
/// produce a sentence a Game Master can act on, and the constraint exists
/// because a check is a race and a constraint is not.
pub async fn complete_lore_repository_connection_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: CompleteConnectionInput,
) -> GraphQLResult<GraphQLLoreRepositoryConnection> {
    let world_id = input.world_id;
    require_world_owner(state, user_id, is_admin, world_id).await?;

    let branch = normalize_branch(input.branch.as_deref())?;
    let directory = normalize_directory(input.directory.as_deref().unwrap_or(DEFAULT_DIRECTORY))?;

    // FR-001.
    if load_connection(state, world_id).await?.is_some() {
        return Err(already_connected());
    }

    let granted = complete_grant(
        state,
        world_id,
        &input.grant_response,
        input.repository_ref.as_deref(),
        user_id,
    )
    .await?;

    // FR-033. Two worlds writing into one directory of one repository would
    // interleave two histories into one tree, and neither owner could tell
    // which commits were theirs.
    if repository_directory_is_claimed(state, &granted.repository_ref, &directory, world_id).await?
    {
        return Err(directory_claimed());
    }

    let now = chrono::Utc::now().naive_utc();
    let row = LoreRepositoryConnection {
        id: Uuid::now_v7(),
        world_id,
        host_kind: granted.host_kind,
        installation_ref: granted.installation_ref,
        repository_ref: granted.repository_ref,
        branch,
        directory,
        // Story 3 only, and off until it exists. A connection that accepted
        // incoming edits on the day it was created would be the first
        // delivery writing to a world's lore.
        incoming_enabled: false,
        // FR-038: null until the Game Master acknowledges the notice, and the
        // background task never picks up a null row. The gate is this column,
        // not the screen that shows the notice.
        notice_acknowledged_at: None,
        state: LoreSyncState::NeverConfigured.as_db_str().to_string(),
        state_reason: None,
        repository_is_public: granted.repository_is_public,
        visibility_checked_at: granted.repository_is_public.map(|_| now),
        deactivated_at: None,
        deactivated_reason: None,
        last_synced_at: None,
        last_written_commit: None,
        created_by: user_id,
        updated_by: user_id,
        created_at: now,
        updated_at: now,
    };

    let inserted = insert_connection(state, row).await?;
    Ok(inserted.into())
}

async fn repository_directory_is_claimed(
    state: &AppState,
    repository_ref: &str,
    directory: &str,
    excluding_world: Uuid,
) -> GraphQLResult<bool> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let repository_ref = repository_ref.to_string();
    let directory = directory.to_string();

    tokio::task::spawn_blocking(move || {
        lore_repository_connections::table
            .filter(lore_repository_connections::repository_ref.eq(repository_ref))
            .filter(lore_repository_connections::directory.eq(directory))
            .filter(lore_repository_connections::world_id.ne(excluding_world))
            .select(lore_repository_connections::id)
            .first::<Uuid>(&mut conn)
            .optional()
            .map(|found| found.is_some())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to check repository directory"))
}

/// Inserts, and translates the two uniqueness constraints back into the same
/// sentences the pre-checks produce. A caller that lost the race between the
/// check and the insert gets the explanation, not a database error.
async fn insert_connection(
    state: &AppState,
    row: LoreRepositoryConnection,
) -> GraphQLResult<LoreRepositoryConnection> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = row.world_id;

    let result = tokio::task::spawn_blocking(move || {
        diesel::insert_into(lore_repository_connections::table)
            .values(row)
            .returning(LoreRepositoryConnection::as_returning())
            .get_result::<LoreRepositoryConnection>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?;

    result.map_err(|err| match err {
        diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::UniqueViolation,
            ref info,
        ) => {
            // Which constraint fired decides which sentence is true, and the
            // constraint name is the only thing that distinguishes them.
            if info
                .constraint_name()
                .is_some_and(|name| name.contains("world"))
            {
                already_connected()
            } else {
                directory_claimed()
            }
        }
        _ => {
            let _ = world_id;
            Error::new("Failed to save the repository connection")
        }
    })
}

/// Testable core of `acknowledgeLoreSyncNotice`.
///
/// Idempotent, and deliberately does not re-stamp an existing acknowledgement:
/// the recorded time is when the Game Master was told, and a second press of a
/// button must not rewrite that.
pub async fn acknowledge_lore_sync_notice_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<GraphQLLoreRepositoryConnection> {
    require_world_owner(state, user_id, is_admin, world_id).await?;

    let existing = load_connection(state, world_id)
        .await?
        .ok_or_else(no_connection)?;

    if existing.notice_acknowledged_at.is_some() {
        return Ok(existing.into());
    }

    let now = chrono::Utc::now().naive_utc();
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let updated = tokio::task::spawn_blocking(move || {
        diesel::update(
            lore_repository_connections::table
                .filter(lore_repository_connections::world_id.eq(world_id)),
        )
        .set((
            lore_repository_connections::notice_acknowledged_at.eq(Some(now)),
            lore_repository_connections::updated_at.eq(now),
            lore_repository_connections::updated_by.eq(user_id),
        ))
        .returning(LoreRepositoryConnection::as_returning())
        .get_result::<LoreRepositoryConnection>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to record the acknowledgement"))?;

    Ok(updated.into())
}

fn no_connection() -> Error {
    Error::new("This world has no repository connection.")
        .extend_with(|_, ext| ext.set("code", "NOT_FOUND"))
}

/// Testable core of `removeLoreRepositoryConnection`.
///
/// Deletes the connection row and nothing else. FR-005: the world's lore is
/// untouched, and the repository's contents are untouched — a removal is the
/// platform forgetting, not a retraction. Nothing is deleted at the host, ever.
///
/// The server-side working clone is a rebuildable cache. Discarding it is
/// `lore_sync::workspace::discard(root, connection_id)`, and this function does
/// not call it because nothing yet configures `root` — T023 owns where clones
/// live, and that call belongs here the moment it does. Nothing depends on the
/// clone's removal for correctness: deleting the row is what stops
/// synchronisation, because the background task iterates rows, and a clone left
/// behind is disk, not behaviour.
///
/// Returns `false` when there was nothing to remove, so a client that asks
/// twice sees "already gone" rather than an error.
/// How a Game Master answers FR-031's divergence.
///
/// Two options, and the absence of a third is the point. Reconciling would mean
/// merging prose, which FR-024 forbids everywhere in this spec — and a
/// synchronisation that silently merged someone's repository edits with their
/// world's would be the one failure mode this feature exists to avoid.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum DivergenceResolution {
    /// Make the repository match the world again, discarding the divergent
    /// history. The world is authoritative (FR-021), and this is the Game
    /// Master saying so on purpose.
    OverwriteRemote,
    /// Stop. The connection is removed and the repository is left exactly as
    /// it is, including whatever diverged.
    AbandonConnection,
}

/// FR-031's explicit choice.
///
/// A run that stopped for divergence does not resume on its own, at any
/// backoff, forever. That is deliberate: the alternative is a system that waits
/// a while and then overwrites work it warned about, which is worse than one
/// that waits indefinitely for a person.
pub async fn resolve_lore_sync_divergence_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    resolution: DivergenceResolution,
) -> GraphQLResult<bool> {
    require_world_owner(state, user_id, is_admin, world_id).await?;

    let existing = load_connection(state, world_id)
        .await?
        .ok_or_else(no_connection)?;

    // An enforcement deactivation is not a divergence a Game Master may
    // resolve. Letting this reopen one would make FR-041a's "cannot be lifted
    // by its owner" false through a side door.
    if existing.state == "deactivated" {
        return Err(Error::new(
            "This connection was deactivated by an administrator and cannot be resumed here.",
        ));
    }

    match resolution {
        DivergenceResolution::AbandonConnection => {
            remove_lore_repository_connection_impl(state, user_id, is_admin, world_id).await
        }
        DivergenceResolution::OverwriteRemote => {
            let now = chrono::Utc::now().naive_utc();
            let mut conn = state
                .db_pool
                .get()
                .map_err(|_| Error::new("Failed to get DB connection"))?;

            // Clearing `last_written_commit` is what actually authorises the
            // overwrite: the next push leases against nothing rather than
            // against a commit the remote no longer has, which is the only
            // way past `--force-with-lease`. The authorisation is therefore a
            // stored fact rather than a flag the pass carries in memory, so a
            // restart cannot lose it and nothing else can invent it.
            tokio::task::spawn_blocking(move || {
                diesel::update(
                    lore_repository_connections::table
                        .filter(lore_repository_connections::world_id.eq(world_id)),
                )
                .set((
                    lore_repository_connections::last_written_commit.eq::<Option<String>>(None),
                    lore_repository_connections::state.eq("working"),
                    lore_repository_connections::state_reason.eq::<Option<String>>(None),
                    lore_repository_connections::updated_at.eq(now),
                    lore_repository_connections::updated_by.eq(user_id),
                ))
                .execute(&mut conn)
            })
            .await
            .map_err(|_| Error::new("Failed to spawn blocking task"))?
            .map_err(|_| Error::new("Failed to record the resolution"))?;

            Ok(true)
        }
    }
}

/// FR-023: nothing is applied until someone with authority says so.
///
/// Owner-level, re-checked here rather than trusted from whenever the change
/// was detected — a change can sit pending for as long as nobody looks at it,
/// and the person who may accept it is the person who has authority *now*.
pub async fn accept_lore_incoming_change_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    change_id: Uuid,
) -> GraphQLResult<bool> {
    require_world_owner(state, user_id, is_admin, world_id).await?;

    let connection = load_connection(state, world_id)
        .await?
        .ok_or_else(no_connection)?;

    // The gate, not a boolean check. `IncomingEnabled` is the only key to a
    // lore write in this feature, and its constructor is where FR-022 and
    // FR-041a are enforced — asking it here rather than re-testing the flags
    // means this path cannot drift away from the rule.
    let gate = crate::lore_sync::incoming::IncomingEnabled::for_connection(&connection)
        .ok_or_else(|| {
            Error::new("This world has not enabled accepting changes from its repository.")
        })?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        crate::lore_sync::incoming::accept(&mut conn, &gate, change_id, user_id)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map(|_| true)
    .map_err(|e| Error::new(e.to_string()))
}

/// FR-026: declining a proposed deletion is safe, and the next pass restores
/// the file. Declining anything else simply leaves the world alone.
pub async fn decline_lore_incoming_change_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    change_id: Uuid,
) -> GraphQLResult<bool> {
    require_world_owner(state, user_id, is_admin, world_id).await?;

    let connection = load_connection(state, world_id)
        .await?
        .ok_or_else(no_connection)?;
    let gate = crate::lore_sync::incoming::IncomingEnabled::for_connection(&connection)
        .ok_or_else(|| {
            Error::new("This world has not enabled accepting changes from its repository.")
        })?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        crate::lore_sync::incoming::decline(&mut conn, &gate, change_id, user_id)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map(|_| true)
    .map_err(|e| Error::new(e.to_string()))
}

/// FR-041a: deactivate a connection against its owner's wishes.
///
/// Administrator-only, and the resulting state is the one a Game Master cannot
/// leave by fixing something (FR-041c). A commitment made to a rights holder
/// that the product cannot carry out is worse than no commitment, so this
/// exists whether or not anything has needed it yet.
///
/// It does not delete the connection. Removal would let the owner simply
/// reconnect, and would also destroy the record of why it was stopped.
pub async fn deactivate_lore_sync_impl(
    state: &AppState,
    is_admin: bool,
    world_id: Uuid,
    reason: String,
) -> GraphQLResult<bool> {
    if !is_admin {
        return Err(Error::new(
            "Only an administrator may deactivate a connection.",
        ));
    }

    let now = chrono::Utc::now().naive_utc();
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let affected = tokio::task::spawn_blocking(move || {
        diesel::update(
            lore_repository_connections::table
                .filter(lore_repository_connections::world_id.eq(world_id)),
        )
        .set((
            lore_repository_connections::state.eq("deactivated"),
            lore_repository_connections::deactivated_at.eq(Some(now)),
            lore_repository_connections::deactivated_reason.eq(Some(reason)),
            lore_repository_connections::updated_at.eq(now),
        ))
        .execute(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to deactivate the connection"))?;

    Ok(affected > 0)
}

pub async fn remove_lore_repository_connection_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<bool> {
    require_world_owner(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let deleted = tokio::task::spawn_blocking(move || {
        diesel::delete(
            lore_repository_connections::table
                .filter(lore_repository_connections::world_id.eq(world_id)),
        )
        .execute(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to remove the repository connection"))?;

    Ok(deleted > 0)
}

#[derive(Default)]
pub struct LoreSyncMutation;

#[Object]
impl LoreSyncMutation {
    /// Begin the grant. Returns where to send the user and what they must be
    /// shown before going (FR-036, FR-036e).
    async fn begin_lore_repository_connection(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<ConnectionGrantHandoff> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        begin_lore_repository_connection_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
        )
        .await
    }

    /// Finish the grant and create the connection.
    ///
    /// Creates nothing when the world already has a connection (FR-001), when
    /// the repository directory is claimed by another world (FR-033), or when
    /// the grant covers more than the one repository being connected
    /// (FR-036a).
    async fn complete_lore_repository_connection(
        &self,
        ctx: &Context<'_>,
        input: CompleteConnectionInput,
    ) -> GraphQLResult<GraphQLLoreRepositoryConnection> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        complete_lore_repository_connection_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            input,
        )
        .await
    }

    /// Record that the Game Master has read FR-037's notice.
    ///
    /// Synchronisation does not begin until this succeeds: a connection whose
    /// `noticeAcknowledgedAt` is null is never picked up (FR-038). The
    /// notice's wording is a client concern; the gate is here, because a
    /// client-side-only gate is not a gate.
    async fn acknowledge_lore_sync_notice(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<GraphQLLoreRepositoryConnection> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        acknowledge_lore_sync_notice_impl(state, auth_user.user_id, auth_user.is_admin, world_id)
            .await
    }

    /// FR-031. Answer a divergence: overwrite the remote, or abandon the
    /// connection. There is deliberately no third option that reconciles,
    /// because reconciling would mean merging prose (FR-024).
    async fn resolve_lore_sync_divergence(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        resolution: DivergenceResolution,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        resolve_lore_sync_divergence_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            resolution,
        )
        .await
    }

    /// FR-023. Apply one pending change, as an ordinary attributed revision.
    async fn accept_lore_incoming_change(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        change_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        accept_lore_incoming_change_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            change_id,
        )
        .await
    }

    /// FR-026. Decline one. A declined deletion is restored by the next pass.
    async fn decline_lore_incoming_change(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        change_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        decline_lore_incoming_change_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            change_id,
        )
        .await
    }

    /// FR-041a. Deactivate a connection against its owner's wishes.
    ///
    /// Administrator-only. Exposed as a mutation rather than left to a database
    /// console because a commitment made to a rights holder that requires
    /// someone to hand-edit a table is a commitment that will not be kept under
    /// time pressure.
    async fn deactivate_lore_sync(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        reason: String,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        deactivate_lore_sync_impl(state, auth_user.is_admin, world_id, reason).await
    }

    /// Remove the connection. Leaves the world's lore and the repository's
    /// contents entirely intact (FR-005).
    async fn remove_lore_repository_connection(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        remove_lore_repository_connection_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
        )
        .await
    }
}

#[cfg(test)]
#[path = "mutations_lore_sync_tests.rs"]
mod tests;
