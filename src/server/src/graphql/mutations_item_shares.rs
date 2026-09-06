//! Spec 013: item sharing and cross-world copy (`createItemShareLink`,
//! `revokeItemShareLink`, `sharedItem`, `copySharedItemToWorld`). Direct
//! structural mirror of `mutations_actor_shares.rs` (research.md §5). See
//! contracts/item-share.md. Reuses the existing `myDmWorlds` query
//! (`queries/user.rs`) as-is — already world-type-agnostic.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::item_permissions::effective_item_permission;
use crate::auth::world_membership::is_dm_of_world;
use crate::graphql::anonymous::caller_id;
use crate::graphql::share_codes::generate_link_code;
use crate::graphql::share_rate_limit as rate_limit;
use crate::graphql::types::{
    ActorPermissionLevel, GraphQLItem, GraphQLItemEffect, GraphQLItemShareLink, SharedItemPreview,
};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{ItemEffect, ItemShare, NewItemEffect, NewItemShare, NewWorldItem, WorldItem};
use crate::schema::{world_item_effects, world_item_shares, world_items};
use crate::state::AppState;

#[derive(InputObject, Debug, Clone)]
pub struct CopySharedItemInput {
    pub share_code: String,
    pub destination_world_id: Uuid,
}

/// See `mutations_actor_shares.rs`'s identical `CopyError` for why this
/// wrapper exists (orphan-rule workaround for the transaction closure).
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
/// ADR-071: an unknown code, a revoked share, a deleted item and a moderated
/// one must be indistinguishable to an outsider, because distinguishing them is
/// a probe — and that matters more now the caller need not have an account. A
/// constant rather than four string literals so they cannot drift apart later,
/// which is exactly how this kind of leak is usually introduced.
pub const UNAVAILABLE: &str = "This share link is no longer available";

fn load_active_share(
    conn: &mut diesel::PgConnection,
    share_code: &str,
) -> Result<ItemShare, String> {
    world_item_shares::table
        .filter(world_item_shares::share_code.eq(share_code))
        .filter(world_item_shares::revoked.eq(false))
        .select(ItemShare::as_select())
        .first::<ItemShare>(conn)
        .map_err(|_| UNAVAILABLE.to_string())
}

/// Testable core of `sharedItem` (FR-033).
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
/// Blocked entirely for a moderated item, so a share can never become a
/// moderation bypass.
pub async fn shared_item_impl(
    state: &AppState,
    caller: &str,
    share_code: String,
) -> GraphQLResult<SharedItemPreview> {
    // ADR-071 (and FR-009c's reasoning): before the lookup, never after.
    if !rate_limit::allow_request(caller) {
        return Err(Error::new(rate_limit::rate_limited_message()));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let (item, effects) = tokio::task::spawn_blocking(move || {
        let share = load_active_share(&mut conn, &share_code)?;

        let item = world_items::table
            .filter(world_items::id.eq(share.item_id))
            .select(WorldItem::as_select())
            .first::<WorldItem>(&mut conn)
            .map_err(|_| UNAVAILABLE.to_string())?;

        let effects = world_item_effects::table
            .filter(world_item_effects::item_id.eq(item.id))
            .order(world_item_effects::sort_order.asc())
            .select(ItemEffect::as_select())
            .load::<ItemEffect>(&mut conn)
            .map_err(|e| format!("Failed to load item effects: {e}"))?;

        Ok::<_, String>((item, effects))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    // Spec 015: a share link must not become a moderation bypass — a
    // disabled item's real content must never leak through this path.
    if crate::moderation::effective_status(state, "world_item", item.id)
        .await?
        .is_some()
    {
        return Err(Error::new(UNAVAILABLE));
    }

    Ok(SharedItemPreview {
        name: item.name,
        description: item.description,
        icon_asset_id: item.icon_asset_id,
        effects: effects.into_iter().map(GraphQLItemEffect::from).collect(),
    })
}

/// Testable core of `ItemShareMutation::create_item_share_link`. Requires
/// effective Owner on the item (FR-022).
/// Testable core of `itemShareLink` — the active share for a item the
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
/// It is scoped to one item the caller already has Owner-level authority
/// over — the same authority needed to mint the link in the first place. It
/// reaches nothing by world, by user, or in aggregate, so nothing becomes
/// discoverable that was not already.
pub async fn item_share_link_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    item_id: Uuid,
) -> GraphQLResult<Option<ItemShare>> {
    // The authority to see a link is the authority to have made one.
    let level = effective_item_permission(state, user_id, is_admin, item_id).await?;
    if level.rank() < ActorPermissionLevel::Owner.rank() {
        return Err(Error::new(
            "Only an Owner-level member may see this item's share link",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_item_shares::table
            .filter(world_item_shares::item_id.eq(item_id))
            .filter(world_item_shares::revoked.eq(false))
            .order(world_item_shares::created_at.desc())
            .select(ItemShare::as_select())
            .first::<ItemShare>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| Error::new(format!("Failed to load the share link: {e}")))
}

pub async fn create_item_share_link_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    item_id: Uuid,
) -> GraphQLResult<ItemShare> {
    let level = effective_item_permission(state, user_id, is_admin, item_id).await?;
    if level.rank() < ActorPermissionLevel::Owner.rank() {
        return Err(Error::new("Only an Owner-level member may share this item"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        let new_share = NewItemShare {
            id: Uuid::now_v7(),
            item_id,
            share_code: generate_link_code(),
            created_by: user_id,
        };

        diesel::insert_into(world_item_shares::table)
            .values(&new_share)
            .returning(ItemShare::as_returning())
            .get_result::<ItemShare>(&mut conn)
            .map_err(|e| format!("Failed to create share link: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `ItemShareMutation::revoke_item_share_link`. Allowed
/// for the link's own creator OR the DM of the item's world (FR-027).
pub async fn revoke_item_share_link_impl(
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
        world_item_shares::table
            .filter(world_item_shares::id.eq(share_id))
            .select(ItemShare::as_select())
            .first::<ItemShare>(&mut conn)
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
    let item_id = share.item_id;
    let world_id = tokio::task::spawn_blocking(move || {
        world_items::table
            .filter(world_items::id.eq(item_id))
            .select(world_items::world_id)
            .first::<Uuid>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load item"))?;

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
        diesel::update(world_item_shares::table.filter(world_item_shares::id.eq(share_id)))
            .set((
                world_item_shares::revoked.eq(true),
                world_item_shares::updated_at.eq(Utc::now().naive_utc()),
            ))
            .execute(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to revoke share link"))?;

    Ok(true)
}

/// Testable core of `ItemShareMutation::copy_shared_item_to_world`.
/// Re-verifies both the share link's validity and the caller's DM-level
/// access on the destination world server-side (FR-024/025/026).
pub async fn copy_shared_item_to_world_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: CopySharedItemInput,
) -> GraphQLResult<(WorldItem, Vec<ItemEffect>)> {
    let destination_world_id = input.destination_world_id;

    if !is_dm_of_world(state, user_id, is_admin, destination_world_id).await? {
        return Err(Error::new(
            "You must hold DM-level access on the destination world to copy an item into it",
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

            let source = world_items::table
                .filter(world_items::id.eq(share.item_id))
                .select(WorldItem::as_select())
                .first::<WorldItem>(conn)
                .map_err(|_| UNAVAILABLE.to_string())?;

            let new_item_row = NewWorldItem {
                world_id: destination_world_id,
                name: source.name.clone(),
                description: source.description.clone(),
                icon_asset_id: None,
                created_by: user_id,
            };

            let created = diesel::insert_into(world_items::table)
                .values(&new_item_row)
                .returning(WorldItem::as_returning())
                .get_result::<WorldItem>(conn)
                .map_err(|e| format!("Failed to create copied item: {e}"))?;

            let source_effects = world_item_effects::table
                .filter(world_item_effects::item_id.eq(source.id))
                .order(world_item_effects::sort_order.asc())
                .select(ItemEffect::as_select())
                .load::<ItemEffect>(conn)
                .map_err(|e| format!("Failed to load source item effects: {e}"))?;

            let mut copied_effects = Vec::with_capacity(source_effects.len());
            for effect in &source_effects {
                let new_effect = NewItemEffect {
                    item_id: created.id,
                    effect_type: effect.effect_type.clone(),
                    formula: effect.formula.clone(),
                    target: effect.target.clone(),
                    trigger_kind: effect.trigger_kind.clone(),
                    sort_order: effect.sort_order,
                };
                let inserted = diesel::insert_into(world_item_effects::table)
                    .values(&new_effect)
                    .returning(ItemEffect::as_returning())
                    .get_result::<ItemEffect>(conn)
                    .map_err(|e| format!("Failed to clone item effect: {e}"))?;
                copied_effects.push(inserted);
            }

            Ok::<_, CopyError>((created, copied_effects))
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e: CopyError| Error::new(e.0))
}

#[derive(Default)]
pub struct ItemShareQuery;

#[async_graphql::Object]
impl ItemShareQuery {
    /// ADR-071: the active share link for a item the caller owns, or null.
    /// **Authenticated**, and scoped to one item the caller already has
    /// authority over — see `item_share_link_impl` for why this is not
    /// enumeration.
    async fn item_share_link(
        &self,
        ctx: &Context<'_>,
        item_id: Uuid,
    ) -> GraphQLResult<Option<GraphQLItemShareLink>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        Ok(
            item_share_link_impl(state, auth_user.user_id, auth_user.is_admin, item_id)
                .await?
                .map(Into::into),
        )
    }

    /// **Deliberately unauthenticated** — ADR-071. Do not add
    /// `authenticated_user(ctx)?` here; it was removed on purpose, and all four
    /// share reads now agree. The caller is identified only to rate-limit them.
    async fn shared_item(
        &self,
        ctx: &Context<'_>,
        share_code: String,
    ) -> GraphQLResult<SharedItemPreview> {
        let state = app_state(ctx)?;
        shared_item_impl(state, &caller_id(ctx), share_code).await
    }
}

#[derive(Default)]
pub struct ItemShareMutation;

#[async_graphql::Object]
impl ItemShareMutation {
    async fn create_item_share_link(
        &self,
        ctx: &Context<'_>,
        item_id: Uuid,
    ) -> GraphQLResult<GraphQLItemShareLink> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        create_item_share_link_impl(state, auth_user.user_id, auth_user.is_admin, item_id)
            .await
            .map(GraphQLItemShareLink::from)
    }

    async fn revoke_item_share_link(
        &self,
        ctx: &Context<'_>,
        share_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        revoke_item_share_link_impl(state, auth_user.user_id, auth_user.is_admin, share_id).await
    }

    async fn copy_shared_item_to_world(
        &self,
        ctx: &Context<'_>,
        input: CopySharedItemInput,
    ) -> GraphQLResult<GraphQLItem> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let (row, effects) =
            copy_shared_item_to_world_impl(state, auth_user.user_id, auth_user.is_admin, input)
                .await?;
        let my_permission_level =
            effective_item_permission(state, auth_user.user_id, auth_user.is_admin, row.id).await?;
        Ok(GraphQLItem::from_row(row, effects, my_permission_level))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_items::{
        CreateItemInput, ItemEffectInput, add_item_effect_impl, create_item_impl,
    };
    use crate::graphql::types::ItemEffectType;
    use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

    /// A distinct caller per test, so the shared rate limiter's window cannot
    /// leak between them: they run concurrently in one process, and a limiter
    /// keyed on a constant would make passing depend on test order.
    fn a_caller() -> String {
        format!("test-{}", Uuid::new_v4())
    }

    /// FR-022: only an Owner-level member (including the DM's implicit
    /// access) may generate a share link.
    #[tokio::test]
    async fn create_share_link_requires_owner_level() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let outsider_id = insert_test_user(&mut conn);
        drop(conn);

        let item = create_item_impl(
            &state,
            owner_id,
            false,
            CreateItemInput {
                world_id,
                name: "Longsword".to_string(),
                description: None,
            },
        )
        .await
        .expect("DM should create item");

        let denied = create_item_share_link_impl(&state, outsider_id, false, item.id).await;
        assert!(
            denied.is_err(),
            "a non-Owner-level caller must not be able to share the item"
        );

        let link = create_item_share_link_impl(&state, owner_id, false, item.id)
            .await
            .expect("the DM (implicit Owner) should be able to share the item");
        assert!(!link.revoked);
    }

    /// FR-025/026: a copy is a fully independent item with cloned effects
    /// and an empty ownership block; destination DM access is re-checked.
    #[tokio::test]
    async fn copy_produces_independent_item_with_cloned_effects() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let source_owner_id = insert_test_user(&mut conn);
        let source_world_id = insert_test_world(&mut conn, source_owner_id);

        let dest_owner_id = insert_test_user(&mut conn);
        let dest_world_id = insert_test_world(&mut conn, dest_owner_id);

        let uninvolved_id = insert_test_user(&mut conn);
        drop(conn);

        let source_item = create_item_impl(
            &state,
            source_owner_id,
            false,
            CreateItemInput {
                world_id: source_world_id,
                name: "Longsword".to_string(),
                description: Some("A fine blade".to_string()),
            },
        )
        .await
        .expect("source DM should create item");

        add_item_effect_impl(
            &state,
            source_owner_id,
            false,
            source_item.id,
            ItemEffectInput {
                effect_type: ItemEffectType::Damage,
                formula: "2d8".to_string(),
                target: "Hit Points".to_string(),
                trigger_kind: None,
                sort_order: Some(0),
            },
        )
        .await
        .expect("effect should be added");

        let link = create_item_share_link_impl(&state, source_owner_id, false, source_item.id)
            .await
            .expect("source DM should be able to share the item");

        let denied = copy_shared_item_to_world_impl(
            &state,
            uninvolved_id,
            false,
            CopySharedItemInput {
                share_code: link.share_code.clone(),
                destination_world_id: dest_world_id,
            },
        )
        .await;
        assert!(
            denied.is_err(),
            "a caller without DM access on the destination must be rejected"
        );

        let (copy, effects) = copy_shared_item_to_world_impl(
            &state,
            dest_owner_id,
            false,
            CopySharedItemInput {
                share_code: link.share_code,
                destination_world_id: dest_world_id,
            },
        )
        .await
        .expect("destination DM should be able to copy the shared item");

        assert_ne!(copy.id, source_item.id, "the copy must have a new identity");
        assert_eq!(copy.world_id, dest_world_id);
        assert_eq!(copy.name, "Longsword");
        assert_eq!(effects.len(), 1, "effects must be cloned onto the copy");
        assert_ne!(
            effects[0].item_id, source_item.id,
            "cloned effect must belong to the copy, not the source"
        );
    }

    /// Spec 015: a moderation-disabled item's share link must not leak
    /// its real content — the share link must not be a takedown bypass.
    #[tokio::test]
    async fn shared_item_is_unavailable_once_moderation_disabled() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let item = create_item_impl(
            &state,
            owner_id,
            false,
            CreateItemInput {
                world_id,
                name: "Infringing Sword".to_string(),
                description: None,
            },
        )
        .await
        .expect("DM should create item");

        let link = create_item_share_link_impl(&state, owner_id, false, item.id)
            .await
            .expect("owner should be able to share the item");

        // Sanity: share works before any takedown.
        assert!(
            shared_item_impl(&state, &a_caller(), link.share_code.clone())
                .await
                .is_ok()
        );

        crate::graphql::mutations_moderation::submit_takedown_notice_impl(
            &state,
            crate::graphql::mutations_moderation::SubmitTakedownNoticeInput {
                entity_type: crate::graphql::types::ModerationEntityType::WorldItem,
                entity_id: item.id,
                claimant_name: "Acme".to_string(),
                claimant_contact: "legal@acme.example".to_string(),
                copyrighted_work_description: "Acme Sourcebook".to_string(),
                infringing_material_location: item.id.to_string(),
                good_faith_statement: true,
                accuracy_statement: true,
                signature: "Jane".to_string(),
            },
        )
        .await
        .expect("valid notice should succeed");

        let result = shared_item_impl(&state, &a_caller(), link.share_code).await;
        assert!(
            result.is_err(),
            "a disabled item's share link must stop serving real content"
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
        drop(conn);

        let item = create_item_impl(
            &state,
            owner_id,
            false,
            CreateItemInput {
                world_id,
                name: "Longsword".to_string(),
                description: None,
            },
        )
        .await
        .expect("DM should create item");

        let link = create_item_share_link_impl(&state, owner_id, false, item.id)
            .await
            .expect("the owner may share");

        revoke_item_share_link_impl(&state, owner_id, false, link.id)
            .await
            .expect("the owner may revoke");

        let revoked = shared_item_impl(&state, &a_caller(), link.share_code)
            .await
            .expect_err("a revoked code must not resolve");
        let unknown = shared_item_impl(&state, &a_caller(), "NOTAREALCODEATALL0".to_string())
            .await
            .expect_err("an unknown code must not resolve");

        assert_eq!(
            revoked.message, unknown.message,
            "the two refusals must be one sentence, or the difference is a probe"
        );
        assert_eq!(revoked.message, UNAVAILABLE);
    }

    /// ADR-071: the read resolves with no session at all. `shared_item_impl`
    /// takes a caller only to rate-limit it, and is reached by a resolver that
    /// never calls `authenticated_user` — this asserts the core behaves that
    /// way rather than asserting the absence of a line of code.
    #[tokio::test]
    async fn the_read_needs_no_account() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let item = create_item_impl(
            &state,
            owner_id,
            false,
            CreateItemInput {
                world_id,
                name: "Longsword".to_string(),
                description: None,
            },
        )
        .await
        .expect("DM should create item");

        let link = create_item_share_link_impl(&state, owner_id, false, item.id)
            .await
            .expect("the owner may share");

        let preview = shared_item_impl(&state, "an-anonymous-visitor-item", link.share_code)
            .await
            .expect("a valid code must resolve for a caller with no account");
        assert_eq!(preview.name, "Longsword");
    }

    /// ADR-071: an unguessable code is unguessable only while the guesses are
    /// bounded, and the account requirement that used to bound them is gone.
    #[tokio::test]
    async fn the_anonymous_read_is_rate_limited() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let item = create_item_impl(
            &state,
            owner_id,
            false,
            CreateItemInput {
                world_id,
                name: "Longsword".to_string(),
                description: None,
            },
        )
        .await
        .expect("DM should create item");

        let link = create_item_share_link_impl(&state, owner_id, false, item.id)
            .await
            .expect("the owner may share");

        let caller = a_caller();
        let mut refused = None;
        for _ in 0..200 {
            if let Err(e) = shared_item_impl(&state, &caller, link.share_code.clone()).await {
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
        drop(conn);

        let item = create_item_impl(
            &state,
            owner_id,
            false,
            CreateItemInput {
                world_id,
                name: "Longsword".to_string(),
                description: None,
            },
        )
        .await
        .expect("DM should create item");

        let link = create_item_share_link_impl(&state, owner_id, false, item.id)
            .await
            .expect("the owner may share");

        let recovered = item_share_link_impl(&state, owner_id, false, item.id)
            .await
            .expect("the owner may read back their own share link")
            .expect("an active share must be found");
        assert_eq!(recovered.share_code, link.share_code);
        assert_eq!(recovered.id, link.id);

        revoke_item_share_link_impl(&state, owner_id, false, link.id)
            .await
            .expect("the owner may revoke");

        let after = item_share_link_impl(&state, owner_id, false, item.id)
            .await
            .expect("reading back after revocation is not an error");
        assert!(
            after.is_none(),
            "a revoked share is not an active share link"
        );
    }
}
