//! Spec 002 (FR-014, FR-015, FR-016, FR-019): shared world-membership
//! authorization guard for canvas image asset reads/writes.
//!
//! Generalizes the inline `world_members` lookup pattern already used in
//! `graphql/mutations_invites.rs` (e.g. `generate_invite_code`'s
//! "Verify user is Owner/GM of the world" check) into one function so
//! both `uploadCanvasImage` (write) and `canvasImageAssetsForScene`
//! (read) enforce the identical rule: a `world_members` row for
//! `(world_id, user_id)` — present only for the world's owner (created
//! at world-creation time) or an invite that has been accepted
//! (`mutations_invites::join_world` is the only other writer of this
//! table) — is required, or the caller is rejected before any other
//! work happens (FR-016).
//!
//! This is a synchronous function so it can run inside the
//! `tokio::task::spawn_blocking` closures both call sites already use
//! for Diesel access (Diesel's `PgConnection` is not `Send`-across-await
//! safe in this codebase's usage pattern), rather than being `async`
//! itself.

use diesel::prelude::*;
use uuid::Uuid;

use async_graphql::{Error, Result as GraphQLResult};

use thunderforge_authz::{Actor, Role};

use crate::schema::{world_members, worlds};
use crate::state::AppState;

#[derive(Debug, Clone, thiserror::Error)]
pub enum WorldMembershipError {
    #[error("user is not a member of this world")]
    NotAMember,
    #[error("database error: {0}")]
    Database(String),
}

impl From<diesel::result::Error> for WorldMembershipError {
    fn from(e: diesel::result::Error) -> Self {
        WorldMembershipError::Database(e.to_string())
    }
}

/// Returns the caller's role string (e.g. `"Owner"`, `"GM"`, `"Player"`)
/// if `user_id` has an accepted `world_members` row for `world_id`, or
/// `WorldMembershipError::NotAMember` otherwise. Callers MUST treat any
/// error from this function as "reject the request" — it is the single
/// authorization gate for canvas asset reads and writes (FR-014..FR-016,
/// FR-019).
///
/// Falls back to `worlds.created_by == user_id` (returning `"Owner"`)
/// when no `world_members` row exists: `create_world`
/// (`src/server/src/graphql.rs`) does not currently insert a
/// `world_members` row for the creator — its RBAC auto-assignment is
/// explicitly commented out as "disabled pending schema" — so
/// `world_members` alone would incorrectly reject a world's own owner.
/// `worlds.created_by` is this codebase's actual, already-enforced
/// ownership source of truth (used directly by every existing
/// scene/wall/shape/token mutation's `scenes::owner_id` chain back to
/// it); this fallback makes `require_world_member` consistent with that
/// existing model rather than silently depending on the disabled
/// auto-assignment being re-enabled first.
pub fn require_world_member(
    conn: &mut PgConnection,
    user_id: Uuid,
    world_id: Uuid,
) -> Result<String, WorldMembershipError> {
    let member_role = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .filter(world_members::user_id.eq(user_id))
        .select(world_members::role)
        .first::<String>(conn)
        .optional()?;

    if let Some(role) = member_role {
        return Ok(role);
    }

    let is_creator = worlds::table
        .filter(worlds::id.eq(world_id))
        .filter(worlds::created_by.eq(user_id))
        .select(worlds::id)
        .first::<Uuid>(conn)
        .optional()?
        .is_some();

    if is_creator {
        Ok("Owner".to_string())
    } else {
        Err(WorldMembershipError::NotAMember)
    }
}

/// Resolve who this caller is in this world, as a value the rules understand.
///
/// The bridge between the database and `thunderforge_authz`: this is the only
/// place a stored role string is turned into a [`Role`], so a spelling the
/// crate does not recognise becomes "no role" exactly once rather than at
/// every call site that used to compare strings by hand.
pub fn actor_in_world(
    conn: &mut PgConnection,
    user_id: uuid::Uuid,
    is_admin: bool,
    world_id: uuid::Uuid,
) -> Actor {
    if is_admin {
        return Actor::site_admin();
    }
    match require_world_member(conn, user_id, world_id) {
        Ok(stored) => Actor {
            role: Role::from_stored(&stored),
            is_site_admin: false,
        },
        // Not a member, or the lookup failed. Either way, nobody.
        Err(_) => Actor::stranger(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_a_member_error_message_is_stable() {
        // Regression guard: callers pattern-match/convert this error into
        // a GraphQL FORBIDDEN — the message shouldn't silently change
        // shape (e.g. via a #[error] format tweak) without a conscious
        // review of every call site.
        let err = WorldMembershipError::NotAMember;
        assert_eq!(err.to_string(), "user is not a member of this world");
    }
}

// ============================================================================
// Spec 027 (T049): `is_dm_of_world` moved here from `auth::actor_permissions`.
//
// It answers a world-level membership question and is already implemented by
// calling `require_world_member` just above — it lived in an actor-specific
// module only because spec 010 needed it first. With 49 call sites spanning
// moderation, dice, items, abilities, lore and world mutations, that location
// was actively misleading; `lore_permissions` had resorted to laundering the
// import with `pub use`.
//
// The signature difference below is deliberate and should not be "harmonised":
// `require_world_member` is synchronous over a borrowed connection, while this
// is async over `AppState`. They answer the same question at two layers.
// ============================================================================

/// Whether `user_id` is "the DM" of `world_id` — holds the world's Owner
/// or GM role (or is an admin). This is the single check every
/// DM-only mutation in spec 010 (actor creation, ownership-block edits,
/// share-link revocation) should call, per research.md §3.
/// Whether this caller **owns** the world, as distinct from running it.
///
/// The three-tier model splits authority in two, and the split is the point:
/// a Game Master carries every power over a world's *content* — tokens,
/// scenes, walls, lights — while the things that end or transfer the world
/// itself stay with the Owner. A co-GM invited to help run a campaign should
/// be able to build the dungeon and should not be able to delete the
/// campaign.
///
/// `is_dm_of_world` answers the first question. This answers the second, and
/// they must not be confused: every caller of this one is a door that cannot
/// be reopened once someone walks through it.
///
/// A site admin is treated as an owner, matching `is_dm_of_world`.
pub async fn is_owner_of_world(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<bool> {
    if is_admin {
        return Ok(true);
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        // Fail closed twice over: an unresolvable caller is not an owner, and
        // a role string this build does not recognise resolves to no role at
        // all rather than to a default one.
        Ok(actor_in_world(&mut conn, user_id, is_admin, world_id).owns_the_world())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

pub async fn is_dm_of_world(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<bool> {
    if is_admin {
        return Ok(true);
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        Ok(actor_in_world(&mut conn, user_id, is_admin, world_id).runs_the_world())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

#[cfg(test)]
mod dm_tests {
    use super::*;
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

    // Spec 027 (T049): these moved here verbatim from `auth::actor_permissions`
    // alongside `is_dm_of_world` itself. The assertions are unchanged — only
    // their location is, so a test sits beside the function it exercises.

    /// FR-021 (research.md §3): a GM-role member counts as DM, same as Owner.
    #[tokio::test]
    async fn gm_role_counts_as_dm() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let gm_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, gm_id, "GM");
        drop(conn);

        let is_dm = is_dm_of_world(&state, gm_id, false, world_id)
            .await
            .expect("dm check should succeed");

        assert!(is_dm, "a GM-role member must count as DM");
    }

    /// A Player-role member is not DM.
    #[tokio::test]
    async fn player_role_is_not_dm() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let is_dm = is_dm_of_world(&state, player_id, false, world_id)
            .await
            .expect("dm check should succeed");

        assert!(!is_dm, "a Player-role member must not count as DM");
    }
}

// ============================================================================
// Content authority: who may author what lives *on* a scene.
//
// The world's three-tier model (Owner / GM / Player) says a GM carries
// authority over content. Until now the scene-scoped content mutations
// (tokens, shapes, walls, lights, map import) never consulted it: they
// authorized on `scenes.owner_id == caller` — the person who happened to
// *create* the scene. A member promoted to GM therefore had no authority on
// a scene the Owner made, and the Owner had none on a scene a GM made. Two
// people both holding GM authority in one world, writing to one scene, saw
// exactly half the writes refused.
//
// `is_dm_of_scene` is the same question `is_dm_of_world` answers, asked one
// level down (from a scene, not a world) and synchronously, so it can run
// inside the `spawn_blocking` closures the content mutations already use for
// Diesel access. It delegates to `require_world_member` rather than
// re-deriving the role, so there is still exactly one implementation of
// "who is the DM here".
//
// Note the boundary this does NOT cross: world-level rights (deleting a
// world, transferring ownership, changing world status) stay Owner-only and
// are gated elsewhere. This function is about content on a scene.
// ============================================================================

/// Whether `user_id` may author content on `scene_id` — i.e. is the Owner or
/// a GM of the world that scene belongs to (or a site admin).
///
/// Synchronous by design: every caller is inside a `spawn_blocking` closure
/// holding a `&mut PgConnection` with no async available, exactly the
/// situation `require_world_member` was made synchronous for.
///
/// Answers `false`, never an error, for a scene that does not exist and for
/// a caller who is not a member. It also answers `false` if the membership
/// lookup itself fails: this gate is fail-closed, and every call site turns
/// `false` into the same "not found or not permitted" refusal, so a database
/// hiccup can only deny a write, never grant one.
pub fn is_dm_of_scene(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    scene_id: Uuid,
) -> Result<bool, diesel::result::Error> {
    use crate::schema::scenes;

    let world_id = scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .select(scenes::world_id)
        .first::<Uuid>(conn)
        .optional()?;

    // A scene that does not exist has no world to be a DM of. Admins are no
    // exception — there is nothing here to authorize against.
    let Some(world_id) = world_id else {
        return Ok(false);
    };

    if is_admin {
        return Ok(true);
    }

    Ok(actor_in_world(conn, user_id, is_admin, world_id).runs_the_world())
}

#[cfg(test)]
mod scene_dm_tests {
    use super::*;
    use crate::test_support::{
        insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
        test_app_state,
    };

    /// The bug this whole change exists for: a member promoted to GM must
    /// carry authority on a scene somebody *else* created. Before
    /// `is_dm_of_scene`, content mutations asked `scenes.owner_id == caller`
    /// and this answered "no".
    #[test]
    fn a_gm_is_dm_of_a_scene_the_owner_created() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let gm_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, gm_id, "GM");

        assert!(is_dm_of_scene(&mut conn, gm_id, false, scene_id).unwrap());
    }

    /// The mirror image, and the half of the bug that was easy to miss: the
    /// world's Owner must carry authority on a scene a GM created.
    #[test]
    fn the_owner_is_dm_of_a_scene_a_gm_created() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let gm_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, gm_id, "GM");
        let scene_id = insert_test_scene(&mut conn, world_id, gm_id);

        assert!(is_dm_of_scene(&mut conn, owner_id, false, scene_id).unwrap());
    }

    /// Players gained nothing. A Player-role member of the same world is not
    /// a content author — this is the assertion that keeps "GM authority"
    /// from quietly becoming "member authority".
    #[test]
    fn a_player_is_not_dm_of_a_scene_in_their_world() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");

        assert!(!is_dm_of_scene(&mut conn, player_id, false, scene_id).unwrap());
    }

    /// A stranger with no membership row at all is refused, and so is a
    /// scene id that matches nothing — a dangling id must not become an
    /// authorization hole.
    #[test]
    fn a_non_member_and_a_nonexistent_scene_are_both_refused() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let stranger_id = insert_test_user(&mut conn);

        assert!(!is_dm_of_scene(&mut conn, stranger_id, false, scene_id).unwrap());
        assert!(!is_dm_of_scene(&mut conn, owner_id, true, Uuid::now_v7()).unwrap());
    }
}
