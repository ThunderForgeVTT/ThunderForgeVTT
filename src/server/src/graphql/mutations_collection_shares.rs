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
use crate::collections::rate_limit;
use crate::collections::resolve::{MemberResolution, resolve_member};
use crate::graphql::share_codes::generate_link_code;
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

/// The caller's identity for rate-limiting purposes, put into the GraphQL
/// context by the public transport handler.
///
/// A newtype rather than a bare `String` so nothing else in the context can be
/// mistaken for it.
#[derive(Clone, Debug)]
pub struct AnonymousCaller(pub String);

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
) -> GraphQLResult<CollectionShare> {
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
        diesel::insert_into(world_collection_shares::table)
            .values(&new_share)
            .returning(CollectionShare::as_returning())
            .get_result::<CollectionShare>(&mut conn)
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
    /// **Deliberately unauthenticated** — ADR-070. Do not add
    /// `authenticated_user(ctx)?` here.
    async fn shared_collection(
        &self,
        ctx: &Context<'_>,
        share_code: String,
    ) -> GraphQLResult<SharedCollectionPreview> {
        let state = app_state(ctx)?;
        // An absent caller identity means the transport did not supply one.
        // Falling back to a shared bucket is the safe way to be wrong: it
        // rate-limits such callers together rather than exempting them.
        let caller = ctx
            .data_opt::<AnonymousCaller>()
            .map(|c| c.0.clone())
            .unwrap_or_else(|| "unknown".to_string());
        shared_collection_impl(state, &caller, share_code).await
    }
}

#[derive(Default)]
pub struct CollectionShareMutation;

#[async_graphql::Object]
impl CollectionShareMutation {
    async fn create_collection_share_link(
        &self,
        ctx: &Context<'_>,
        collection_id: Uuid,
    ) -> GraphQLResult<GraphQLCollectionShareLink> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        create_collection_share_link_impl(state, user.user_id, user.is_admin, collection_id)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_collections::{
        AddCollectionMemberInput, CreateCollectionInput, add_collection_member_impl,
        create_collection_impl, delete_collection_impl,
    };
    use crate::test_support::*;

    struct Fixture {
        state: AppState,
        owner_id: Uuid,
        world_id: Uuid,
        scene_id: Uuid,
        item_id: Uuid,
        ability_id: Uuid,
        world_name: String,
    }

    fn fixture() -> Fixture {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("connection");

        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let item_id = insert_test_item(&mut conn, world_id, owner_id);
        let ability_id = insert_test_ability(&mut conn, world_id, owner_id);

        let world_name: String = crate::schema::worlds::table
            .filter(crate::schema::worlds::id.eq(world_id))
            .select(crate::schema::worlds::name)
            .first(&mut conn)
            .expect("world name");

        Fixture {
            state,
            owner_id,
            world_id,
            scene_id,
            item_id,
            ability_id,
            world_name,
        }
    }

    /// A collection holding an item, an ability and a scene, already shared.
    async fn shared_fixture(f: &Fixture) -> (Uuid, CollectionShare) {
        let collection = create_collection_impl(
            &f.state,
            f.owner_id,
            false,
            CreateCollectionInput {
                world_id: f.world_id,
                name: "The Haunted Manor".to_string(),
                description: Some("Rooms, curses and the reasons why.".to_string()),
            },
        )
        .await
        .expect("created");

        for (member_type, member_id) in [
            ("item", f.item_id),
            ("ability", f.ability_id),
            ("scene", f.scene_id),
        ] {
            add_collection_member_impl(
                &f.state,
                f.owner_id,
                false,
                AddCollectionMemberInput {
                    collection_id: collection.id,
                    member_type: member_type.to_string(),
                    member_id,
                },
            )
            .await
            .expect("added");
        }

        let share = create_collection_share_link_impl(&f.state, f.owner_id, false, collection.id)
            .await
            .expect("shared");
        (collection.id, share)
    }

    /// A rate-limit bucket nothing else in the process shares, so one test
    /// exhausting its budget cannot fail another.
    fn a_caller() -> String {
        format!("test-{}", Uuid::new_v4())
    }

    /// FR-008: the code is 20 uppercase hex characters, v4-derived.
    #[tokio::test]
    async fn a_share_link_carries_an_unguessable_code() {
        let f = fixture();
        let (_, share) = shared_fixture(&f).await;
        assert_eq!(share.share_code.len(), 20);
        assert_eq!(share.share_code, share.share_code.to_uppercase());
        assert!(!share.revoked);
    }

    /// FR-009a: no session. This is the whole of ADR-070 in one assertion —
    /// `shared_collection_impl` takes no user and asks for none.
    #[tokio::test]
    async fn a_collection_previews_without_any_account() {
        let f = fixture();
        let (_, share) = shared_fixture(&f).await;

        let preview = shared_collection_impl(&f.state, &a_caller(), share.share_code)
            .await
            .expect("an anonymous caller may read a shared collection");

        assert_eq!(preview.name, "The Haunted Manor");
        assert_eq!(preview.members.len(), 3);
        assert_eq!(preview.withheld_count, 0);
    }

    /// US4 scenario 1: how many of each kind, before copying.
    #[tokio::test]
    async fn the_preview_says_how_many_of_each_kind() {
        let f = fixture();
        let (_, share) = shared_fixture(&f).await;

        let preview = shared_collection_impl(&f.state, &a_caller(), share.share_code)
            .await
            .expect("preview");

        let mut kinds: Vec<(String, i32)> = preview
            .counts_by_type
            .into_iter()
            .map(|c| (c.member_type, c.count))
            .collect();
        kinds.sort();
        assert_eq!(
            kinds,
            vec![
                ("ability".to_string(), 1),
                ("item".to_string(), 1),
                ("scene".to_string(), 1),
            ]
        );
    }

    /// FR-009 / SC-007a: the preview reveals nothing about the source world.
    ///
    /// Serialised and searched rather than field-by-field, so a field added
    /// later that happens to carry a world identifier is caught by a test
    /// nobody remembered to update.
    #[tokio::test]
    async fn the_preview_reveals_nothing_about_the_source_world() {
        let f = fixture();
        let (_, share) = shared_fixture(&f).await;

        let preview = shared_collection_impl(&f.state, &a_caller(), share.share_code)
            .await
            .expect("preview");

        let rendered = format!("{preview:?}");
        assert!(
            !rendered.contains(&f.world_id.to_string()),
            "the world id must not appear in the preview: {rendered}"
        );
        assert!(
            !rendered.contains(&f.world_name),
            "the world name must not appear in the preview: {rendered}"
        );
        assert!(
            !rendered.contains(&f.owner_id.to_string()),
            "the owner's id must not appear in the preview: {rendered}"
        );
    }

    /// FR-010 + FR-009d: revoking makes the link unavailable, and the sentence
    /// is the same one an unknown code produces.
    #[tokio::test]
    async fn a_revoked_share_is_indistinguishable_from_a_code_that_never_existed() {
        let f = fixture();
        let (_, share) = shared_fixture(&f).await;

        shared_collection_impl(&f.state, &a_caller(), share.share_code.clone())
            .await
            .expect("works before revocation");

        assert!(
            revoke_collection_share_link_impl(&f.state, f.owner_id, false, share.id)
                .await
                .expect("revoked")
        );

        let revoked_error = shared_collection_impl(&f.state, &a_caller(), share.share_code)
            .await
            .expect_err("a revoked link must not resolve");
        let unknown_error =
            shared_collection_impl(&f.state, &a_caller(), "NOTAREALCODEATALL0".to_string())
                .await
                .expect_err("an unknown code must not resolve");

        assert_eq!(
            revoked_error.message, unknown_error.message,
            "a revoked share and an unknown code must be indistinguishable (FR-009d)"
        );
        assert_eq!(revoked_error.message, UNAVAILABLE);
    }

    /// US2 scenario 4 + FR-009d: deleting the collection behaves as revoked,
    /// and is likewise indistinguishable.
    #[tokio::test]
    async fn a_deleted_collection_reads_the_same_as_an_unknown_code() {
        let f = fixture();
        let (collection_id, share) = shared_fixture(&f).await;

        delete_collection_impl(&f.state, f.owner_id, false, collection_id)
            .await
            .expect("deleted");

        let error = shared_collection_impl(&f.state, &a_caller(), share.share_code)
            .await
            .expect_err("a deleted collection's link must not resolve");
        assert_eq!(error.message, UNAVAILABLE);
    }

    /// FR-011: a revoked share leaves the collection and its artifacts alone.
    /// Revocation ends the link, not the content.
    #[tokio::test]
    async fn revoking_touches_neither_the_collection_nor_its_artifacts() {
        use crate::schema::world_items;

        let f = fixture();
        let (collection_id, share) = shared_fixture(&f).await;

        revoke_collection_share_link_impl(&f.state, f.owner_id, false, share.id)
            .await
            .expect("revoked");

        let mut conn = f.state.db_pool.get().expect("connection");
        let collection_survives: i64 = world_collections::table
            .filter(world_collections::id.eq(collection_id))
            .count()
            .get_result(&mut conn)
            .expect("count");
        assert_eq!(collection_survives, 1);

        let item_survives: i64 = world_items::table
            .filter(world_items::id.eq(f.item_id))
            .count()
            .get_result(&mut conn)
            .expect("count");
        assert_eq!(item_survives, 1);
    }

    /// A collection may be shared, revoked and shared again with a new code —
    /// which is why the share is a separate row from the collection.
    #[tokio::test]
    async fn a_collection_can_be_shared_again_after_revocation() {
        let f = fixture();
        let (collection_id, first) = shared_fixture(&f).await;

        revoke_collection_share_link_impl(&f.state, f.owner_id, false, first.id)
            .await
            .expect("revoked");

        let second = create_collection_share_link_impl(&f.state, f.owner_id, false, collection_id)
            .await
            .expect("shared again");

        assert_ne!(first.share_code, second.share_code, "a new code each time");
        shared_collection_impl(&f.state, &a_caller(), second.share_code)
            .await
            .expect("the new link works");
        shared_collection_impl(&f.state, &a_caller(), first.share_code)
            .await
            .expect_err("the old link stays dead");
    }

    /// FR-001b at share time: a member restricted after being added blocks the
    /// share, and says which one.
    #[tokio::test]
    async fn a_member_restricted_after_being_added_blocks_a_new_share() {
        use crate::schema::world_abilities;

        let f = fixture();
        let collection = create_collection_impl(
            &f.state,
            f.owner_id,
            false,
            CreateCollectionInput {
                world_id: f.world_id,
                name: "Restricted later".to_string(),
                description: None,
            },
        )
        .await
        .expect("created");

        add_collection_member_impl(
            &f.state,
            f.owner_id,
            false,
            AddCollectionMemberInput {
                collection_id: collection.id,
                member_type: "ability".to_string(),
                member_id: f.ability_id,
            },
        )
        .await
        .expect("added while unrestricted");

        let mut conn = f.state.db_pool.get().expect("connection");
        diesel::update(world_abilities::table.filter(world_abilities::id.eq(f.ability_id)))
            .set(world_abilities::gm_only.eq(true))
            .execute(&mut conn)
            .expect("restrict it");
        drop(conn);

        let error = create_collection_share_link_impl(&f.state, f.owner_id, false, collection.id)
            .await
            .expect_err("a restricted member must block the share");
        assert!(
            error.message.contains("Game Master"),
            "the refusal must name the reason, got: {}",
            error.message
        );
    }

    /// FR-024: every member withheld reports that nothing is available, rather
    /// than presenting an empty collection as complete.
    #[tokio::test]
    async fn a_collection_whose_members_all_vanished_says_nothing_is_available() {
        use crate::schema::{scenes, world_abilities, world_items};

        let f = fixture();
        let (_, share) = shared_fixture(&f).await;

        let mut conn = f.state.db_pool.get().expect("connection");
        diesel::delete(world_items::table.filter(world_items::id.eq(f.item_id)))
            .execute(&mut conn)
            .expect("delete the item");
        diesel::delete(world_abilities::table.filter(world_abilities::id.eq(f.ability_id)))
            .execute(&mut conn)
            .expect("delete the ability");
        diesel::delete(scenes::table.filter(scenes::scene_id.eq(f.scene_id)))
            .execute(&mut conn)
            .expect("delete the scene");
        drop(conn);

        let error = shared_collection_impl(&f.state, &a_caller(), share.share_code)
            .await
            .expect_err("an entirely withheld collection must not read as complete");
        assert!(error.message.contains("Nothing"), "got: {}", error.message);
    }

    /// FR-022: one member gone leaves the rest, and the absence is a count
    /// rather than a name.
    #[tokio::test]
    async fn one_missing_member_is_counted_never_named() {
        use crate::schema::world_items;

        let f = fixture();
        let (_, share) = shared_fixture(&f).await;

        let item_name: String = {
            let mut conn = f.state.db_pool.get().expect("connection");
            let name = world_items::table
                .filter(world_items::id.eq(f.item_id))
                .select(world_items::name)
                .first::<String>(&mut conn)
                .expect("item name");
            diesel::delete(world_items::table.filter(world_items::id.eq(f.item_id)))
                .execute(&mut conn)
                .expect("delete the item");
            name
        };

        let preview = shared_collection_impl(&f.state, &a_caller(), share.share_code)
            .await
            .expect("the collection still opens");

        assert_eq!(preview.members.len(), 2, "the rest are still there");
        assert_eq!(preview.withheld_count, 1);
        assert!(
            !format!("{preview:?}").contains(&item_name),
            "the withheld member must not be named"
        );
    }

    /// FR-009c: the anonymous read is rate limited, and the refusal reveals
    /// nothing about the code that was tried.
    #[tokio::test]
    async fn the_anonymous_read_is_rate_limited() {
        let f = fixture();
        let (_, share) = shared_fixture(&f).await;
        let caller = a_caller();

        let mut refused = None;
        for _ in 0..200 {
            if let Err(e) =
                shared_collection_impl(&f.state, &caller, share.share_code.clone()).await
            {
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
            "the rate-limit refusal must not masquerade as a bad code"
        );
    }

    /// An empty collection cannot be shared. A link to nothing is a link that
    /// reads as broken.
    #[tokio::test]
    async fn an_empty_collection_cannot_be_shared() {
        let f = fixture();
        let collection = create_collection_impl(
            &f.state,
            f.owner_id,
            false,
            CreateCollectionInput {
                world_id: f.world_id,
                name: "Empty".to_string(),
                description: None,
            },
        )
        .await
        .expect("created");

        create_collection_share_link_impl(&f.state, f.owner_id, false, collection.id)
            .await
            .expect_err("an empty collection must not be shareable");
    }
}
