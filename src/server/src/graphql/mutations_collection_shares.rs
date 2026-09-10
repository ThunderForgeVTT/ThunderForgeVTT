//! Spec 026: sharing a collection, revoking that share, and the **anonymous**
//! read of one.
//!
//! Governed by ADR-069 (the DMCA determination, which accepts a stated risk)
//! and ADR-070 (the anonymous read path).
//!
//! # Three invariants live in this file
//!
//! **`shared_collection` must not authenticate.** This is the deliberate
//! divergence ADR-070 exists to record: `sharedAbility`, `sharedItem` and
//! `sharedActor` each call `authenticated_user(ctx)?` — waiving the
//! *membership* check but not the session. This one waives both. A future
//! reader "restoring consistency" with the other three would be reverting a
//! decision, not fixing an omission.
//!
//! **Every refusal says the same thing** (FR-009d). An unknown code, a revoked
//! share, a deleted collection and a collection with no active share are
//! indistinguishable to an outsider, because distinguishing them is a probe.
//!
//! **Nothing here lists anything** (FR-020). There is no query that reaches
//! shares by world, by user, or in aggregate. ADR-069's determination that a
//! link-shared collection is not a centralized public repository rests on
//! there being nothing to enumerate.

use async_graphql::{Context, Error, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::is_dm_of_world;
use crate::collections::resolve::{MemberResolution, resolve_member};
use crate::graphql::anonymous::caller_id;
use crate::graphql::share_codes::generate_link_code;
use crate::graphql::share_rate_limit as rate_limit;
use crate::graphql::{app_state, authenticated_user};
use crate::models::{Collection, CollectionMember, CollectionShare, NewCollectionShare};
use crate::schema::{world_collection_members, world_collection_shares, world_collections};
use crate::state::AppState;

/// The one sentence every failed lookup produces.
///
/// FR-009d: an outsider must not be able to tell an unknown code from a
/// revoked share from a deleted collection. Four states, one sentence — and it
/// is a constant rather than four string literals so that they cannot drift
/// apart later, which is exactly how this kind of leak is usually introduced.
pub const UNAVAILABLE: &str = "This collection link is no longer available";

#[derive(SimpleObject, Debug, Clone)]
pub struct SharedCollectionMemberPreview {
    pub member_type: String,
    pub name: String,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct CollectionTypeCount {
    pub member_type: String,
    pub count: i32,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct SharedCollectionPreview {
    pub name: String,
    pub description: Option<String>,
    pub members: Vec<SharedCollectionMemberPreview>,
    /// US4 scenario 1: how many of each kind, before copying.
    pub counts_by_type: Vec<CollectionTypeCount>,
    /// FR-022: **a number, never a name.** Reproducing the title of a
    /// taken-down artifact in the sentence explaining that it was taken down
    /// would defeat the takedown.
    pub withheld_count: i32,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLCollectionShareLink {
    pub id: Uuid,
    pub collection_id: Uuid,
    pub share_code: String,
    pub revoked: bool,
}

impl From<CollectionShare> for GraphQLCollectionShareLink {
    fn from(row: CollectionShare) -> Self {
        Self {
            id: row.id,
            collection_id: row.collection_id,
            share_code: row.share_code,
            revoked: row.revoked,
        }
    }
}

/// The active share for this code, or the one refusal sentence.
pub fn load_active_share(
    conn: &mut PgConnection,
    share_code: &str,
) -> Result<CollectionShare, String> {
    world_collection_shares::table
        .filter(world_collection_shares::share_code.eq(share_code))
        .filter(world_collection_shares::revoked.eq(false))
        .select(CollectionShare::as_select())
        .first::<CollectionShare>(conn)
        .map_err(|_| UNAVAILABLE.to_string())
}

/// Testable core of `collectionShareLink` (FR-010a): the active share for a
/// collection **you own**, or `None`.
///
/// # Why this is not the enumeration FR-020 forbids
///
/// FR-020 bars browsing, searching or counting collections "beyond a user's
/// own". This takes one collection id, verifies the caller is its creator or a
/// DM of its world through the same check every mutation here uses, and
/// returns that one collection's share. Nothing is listed, nothing is
/// discoverable, and a caller learns only about a collection they could
/// already read.
///
/// # Why it exists
///
/// FR-010 says the owner must be able to revoke. Without a read path that was
/// only true inside the browser session that minted the link: the code was
/// displayed once, and closing the tab removed the owner's ability to revoke
/// it permanently. The three shipped single-artifact shares still have that
/// defect; FR-009e holds the decision to change them.
pub async fn collection_share_link_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    collection_id: Uuid,
) -> GraphQLResult<Option<CollectionShare>> {
    let (world_id, created_by) = collection_world_and_owner(state, collection_id).await?;

    // Same refusal as everywhere else in this module: a caller with no
    // authority is told the collection does not exist rather than that it
    // exists and is not theirs.
    if created_by != user_id && !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("Collection not found"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    world_collection_shares::table
        .filter(world_collection_shares::collection_id.eq(collection_id))
        .filter(world_collection_shares::revoked.eq(false))
        .order(world_collection_shares::created_at.desc())
        .select(CollectionShare::as_select())
        .first::<CollectionShare>(&mut conn)
        .optional()
        .map_err(|e| Error::new(format!("Failed to load the share link: {e}")))
}

/// Testable core of `createCollectionShareLink` (FR-006, FR-008).
///
/// Re-checks **every member's restriction at share time**, not only at add
/// time. The shipped ability path re-checks for the same reason it gives:
/// sharing "is the one path that escapes the world". A member restricted after
/// it was added would otherwise be published by a share created afterwards.
pub async fn create_collection_share_link_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    collection_id: Uuid,
    attestation: &crate::publishing::AttestationInput,
) -> GraphQLResult<CollectionShare> {
    // Spec 040 FR-026: an instance with no contact for copyright notices
    // publishes nothing beyond a world. Asked first, before ownership, so a
    // misconfigured instance answers the same sentence to every caller
    // instead of leaking which of them owns what — and asked here, in the
    // impl, so a caller that bypasses the page is refused too (spec 039
    // FR-011, FR-014). Reading an existing share is deliberately not gated.
    crate::readiness::may_publish_beyond_world(state)
        .await
        .map_err(Error::new)?;

    let (world_id, created_by) = collection_world_and_owner(state, collection_id).await?;

    // The collection's creator, or a DM of its world.
    if created_by != user_id && !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("Collection not found"));
    }

    let members = load_members(state, collection_id).await?;
    if members.is_empty() {
        return Err(Error::new(
            "This collection is empty. Add something to it before sharing.",
        ));
    }

    for member in &members {
        if let Some(reason) = crate::collections::membership::restriction_reason(
            state,
            &member.member_type,
            member.member_id,
        )
        .await?
        {
            return Err(Error::new(format!(
                "This collection cannot be shared yet. {reason}"
            )));
        }
    }

    // Spec 039 FR-011/FR-014. Last of the refusals, and deliberately so: it is
    // the only one whose message tells the caller to reload a page, and telling
    // somebody that before telling them the collection is empty would send them
    // round a loop that cannot fix anything.
    //
    // The gate writes nothing. What it returns is written below, inside the
    // transaction that mints the code.
    let pending = crate::publishing::require_attestation(
        state,
        user_id,
        crate::attestation::PublishableKind::Collection,
        collection_id,
        Some(world_id),
        attestation,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let new_share = NewCollectionShare {
        id: Uuid::now_v7(),
        collection_id,
        share_code: generate_link_code(),
        created_by: user_id,
    };

    tokio::task::spawn_blocking(move || {
        // One transaction: there is no interleaving in which a share link
        // exists without its attestation, and none in which an attestation
        // exists for a share that failed (FR-011).
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let share = diesel::insert_into(world_collection_shares::table)
                .values(&new_share)
                .returning(CollectionShare::as_returning())
                .get_result::<CollectionShare>(conn)?;
            crate::attestation::record_sync(conn, &pending, share.id)?;
            Ok(share)
        })
        .map_err(|e| format!("Failed to create share link: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `revokeCollectionShareLink` (FR-010, FR-011).
///
/// A soft flag, never a delete. A deleted row could not distinguish "revoked"
/// from "never existed" — and while FR-009d requires those to look the same to
/// an *outsider*, the owner's own interface needs to know the share exists in
/// order to show it as revoked.
pub async fn revoke_collection_share_link_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    share_id: Uuid,
) -> GraphQLResult<bool> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let (created_by, collection_id) = tokio::task::spawn_blocking(move || {
        world_collection_shares::table
            .filter(world_collection_shares::id.eq(share_id))
            .select((
                world_collection_shares::created_by,
                world_collection_shares::collection_id,
            ))
            .first::<(Uuid, Uuid)>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load share link"))?
    .ok_or_else(|| Error::new("Share link not found"))?;

    let (world_id, _) = collection_world_and_owner(state, collection_id).await?;

    if created_by != user_id && !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("You may not revoke this share link"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::update(
            world_collection_shares::table.filter(world_collection_shares::id.eq(share_id)),
        )
        .set((
            world_collection_shares::revoked.eq(true),
            world_collection_shares::updated_at.eq(chrono::Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .map(|rows| rows > 0)
        .map_err(|e| format!("Failed to revoke share link: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `sharedCollection` (FR-009a, FR-009c, FR-009d, FR-022,
/// FR-024).
///
/// **Anonymous.** The caller is identified only for rate limiting.
///
/// Reveals nothing about the source world — not its id, not its name, not its
/// other content (FR-009, FR-009d). Note what is *not* returned below: the
/// collection row carries `world_id`, `created_by` and `updated_by`, and none
/// of them reach the preview.
pub async fn shared_collection_impl(
    state: &AppState,
    caller: &str,
    share_code: String,
) -> GraphQLResult<SharedCollectionPreview> {
    // FR-009c, before the lookup. An unguessable code is unguessable only
    // while the number of guesses is bounded.
    if !rate_limit::allow_request(caller) {
        return Err(Error::new(rate_limit::rate_limited_message()));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let collection = tokio::task::spawn_blocking(move || {
        let share = load_active_share(&mut conn, &share_code)?;
        world_collections::table
            .filter(world_collections::id.eq(share.collection_id))
            .select(Collection::as_select())
            .first::<Collection>(&mut conn)
            .map_err(|_| UNAVAILABLE.to_string())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    let members = load_members(state, collection.id).await?;

    let mut visible = Vec::new();
    let mut withheld_count = 0i32;
    for member in &members {
        match resolve_member(state, member).await? {
            MemberResolution::Visible { name } => visible.push(SharedCollectionMemberPreview {
                member_type: member.member_type.clone(),
                name,
            }),
            // Withheld and Gone read identically to a viewer. The distinction
            // is for the copy path's fidelity notes, not for a stranger.
            MemberResolution::Withheld | MemberResolution::Gone => withheld_count += 1,
        }
    }

    // FR-024: a collection whose every member is withheld reports that nothing
    // is available, rather than presenting an empty collection as complete.
    // This may say so distinctly, unlike the four refusals above — reaching
    // this point already required a valid code, so it reveals nothing a
    // prober did not have.
    if visible.is_empty() {
        return Err(Error::new(
            "Nothing in this collection is available right now",
        ));
    }

    let mut counts_by_type: Vec<CollectionTypeCount> = Vec::new();
    for member_type in crate::collections::MEMBER_TYPES {
        let count = visible
            .iter()
            .filter(|m| m.member_type == *member_type)
            .count() as i32;
        if count > 0 {
            counts_by_type.push(CollectionTypeCount {
                member_type: (*member_type).to_string(),
                count,
            });
        }
    }

    Ok(SharedCollectionPreview {
        name: collection.name,
        description: collection.description,
        members: visible,
        counts_by_type,
        withheld_count,
    })
}

/// A collection's world and creator, or "not found".
async fn collection_world_and_owner(
    state: &AppState,
    collection_id: Uuid,
) -> GraphQLResult<(Uuid, Uuid)> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_collections::table
            .filter(world_collections::id.eq(collection_id))
            .select((world_collections::world_id, world_collections::created_by))
            .first::<(Uuid, Uuid)>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load collection"))?
    .ok_or_else(|| Error::new("Collection not found"))
}

/// Every membership row of a collection, in order.
pub async fn load_members(
    state: &AppState,
    collection_id: Uuid,
) -> GraphQLResult<Vec<CollectionMember>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_collection_members::table
            .filter(world_collection_members::collection_id.eq(collection_id))
            .order(world_collection_members::sort_order.asc())
            .select(CollectionMember::as_select())
            .load::<CollectionMember>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load collection members"))
}

#[derive(Default)]
pub struct CollectionShareQuery;

#[async_graphql::Object]
impl CollectionShareQuery {
    /// FR-010a: the active share link for a collection the caller owns, or
    /// null. **Authenticated**, and scoped to one collection the caller
    /// already has authority over — see `collection_share_link_impl` for why
    /// this is not the enumeration FR-020 forbids.
    async fn collection_share_link(
        &self,
        ctx: &Context<'_>,
        collection_id: Uuid,
    ) -> GraphQLResult<Option<GraphQLCollectionShareLink>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        Ok(
            collection_share_link_impl(state, user.user_id, user.is_admin, collection_id)
                .await?
                .map(Into::into),
        )
    }

    /// **Deliberately unauthenticated** — ADR-070. Do not add
    /// `authenticated_user(ctx)?` here.
    async fn shared_collection(
        &self,
        ctx: &Context<'_>,
        share_code: String,
    ) -> GraphQLResult<SharedCollectionPreview> {
        let state = app_state(ctx)?;
        shared_collection_impl(state, &caller_id(ctx), share_code).await
    }
}

#[derive(Default)]
pub struct CollectionShareMutation;

#[async_graphql::Object]
impl CollectionShareMutation {
    /// Spec 039 FR-001/FR-002: `attestation` is **non-null**. A nullable
    /// argument is a requirement satisfied by omission, which is the shape this
    /// feature exists to remove.
    async fn create_collection_share_link(
        &self,
        ctx: &Context<'_>,
        collection_id: Uuid,
        attestation: crate::publishing::AttestationInput,
    ) -> GraphQLResult<GraphQLCollectionShareLink> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        create_collection_share_link_impl(
            state,
            user.user_id,
            user.is_admin,
            collection_id,
            &attestation,
        )
        .await
        .map(Into::into)
    }

    async fn revoke_collection_share_link(
        &self,
        ctx: &Context<'_>,
        share_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        revoke_collection_share_link_impl(state, user.user_id, user.is_admin, share_id).await
    }

    /// **Authenticated**, unlike `sharedCollection`. Viewing and copying are
    /// different acts with different requirements (FR-009b), and this is
    /// exactly where they diverge.
    async fn copy_shared_collection_to_world(
        &self,
        ctx: &Context<'_>,
        share_code: String,
        destination_world_id: Uuid,
    ) -> GraphQLResult<crate::collections::copy::CopyReceipt> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        crate::collections::copy::copy_shared_collection_to_world_impl(
            state,
            user.user_id,
            user.is_admin,
            share_code,
            destination_world_id,
        )
        .await
    }
}

/// The guard that keeps the publishing gate true for a fifth content type,
/// and the fixtures the four share modules' tests share.
///
/// Declared here, by path, because `graphql.rs` is the module list every
/// feature edits at once and this is a test-only sibling of the four files
/// that need it. See the module's own documentation.
#[cfg(test)]
#[path = "publishing_gate.rs"]
pub(crate) mod publishing_gate;

#[cfg(test)]
#[path = "mutations_collection_shares_tests.rs"]
mod tests;
