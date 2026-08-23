//! Spec 012: world lore wiki queries — `worldLoreEntries`, `loreEntry`,
//! `loreLinkTargets` (US2's `[[`-autocomplete), `loreEntryRevisions`
//! (US5). See `specs/012-lore-wiki/contracts/lore-crud.md`,
//! `lore-revisions.md`.

use async_graphql::{Context, Enum, Error, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::lore_permissions::require_lore_permission;
use crate::graphql::types::{ActorPermissionLevel, GraphQLLoreEntry, GraphQLLoreRevision};
use crate::graphql::{app_state, authenticated_user, require_visible_world};
use crate::models::{LoreEntry, LoreRevision};
use crate::schema::{world_actors, world_lore_entries, world_lore_links, world_lore_revisions};
use crate::state::AppState;

/// Shared by `GraphQLLoreEntry::rendered_html` and
/// `GraphQLLoreRevision::rendered_html` — takes a `Context` directly
/// since both call sites are `#[ComplexObject]` field resolvers.
pub async fn render_lore_content(
    ctx: &Context<'_>,
    world_id: Uuid,
    content: &str,
) -> GraphQLResult<String> {
    let state = app_state(ctx)?;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let content = content.to_string();
    tokio::task::spawn_blocking(move || {
        let (rewritten, links) =
            crate::markdown::links::extract_and_resolve(&mut conn, world_id, &content)
                .map_err(|_| Error::new("Failed to resolve lore links"))?;
        let html = crate::markdown::render_to_safe_html(&rewritten);
        Ok::<_, Error>(crate::markdown::links::substitute_placeholders_into_html(&html, &links))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// The world a lore entry belongs to — used by
/// `GraphQLLoreRevision::rendered_html` to resolve links against the
/// right world's entries/actors.
pub async fn world_id_for_lore_entry(ctx: &Context<'_>, lore_entry_id: Uuid) -> GraphQLResult<Uuid> {
    let state = app_state(ctx)?;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_lore_entries::table
            .filter(world_lore_entries::id.eq(lore_entry_id))
            .select(world_lore_entries::world_id)
            .first::<Uuid>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Lore entry not found"))
}

/// Every lore entry whose body currently contains a resolved in-text
/// link to `target_lore_entry_id` (FR-006).
pub async fn lore_entries_linking_to(
    ctx: &Context<'_>,
    target_lore_entry_id: Uuid,
) -> GraphQLResult<Vec<GraphQLLoreEntry>> {
    let state = app_state(ctx)?;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let rows = tokio::task::spawn_blocking(move || {
        world_lore_links::table
            .filter(world_lore_links::target_lore_entry_id.eq(target_lore_entry_id))
            .inner_join(
                world_lore_entries::table
                    .on(world_lore_links::source_lore_entry_id.eq(world_lore_entries::id)),
            )
            .select(LoreEntry::as_select())
            .load::<LoreEntry>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load linked-from entries"))?;

    // Spec 015: a disabled source entry must not leak its title/content
    // through a target's "linked from" list.
    let rows = crate::moderation::filter_visible(state, "world_lore_entry", rows, |e| e.id).await?;
    Ok(rows.into_iter().map(GraphQLLoreEntry::from).collect())
}

/// Every lore entry whose body currently contains a resolved in-text
/// link to `target_actor_id` — the actor-side counterpart of
/// `lore_entries_linking_to`, called from `queries::actor`.
pub async fn lore_entries_linking_to_actor(
    state: &AppState,
    target_actor_id: Uuid,
) -> GraphQLResult<Vec<GraphQLLoreEntry>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let rows = tokio::task::spawn_blocking(move || {
        world_lore_links::table
            .filter(world_lore_links::target_actor_id.eq(target_actor_id))
            .inner_join(
                world_lore_entries::table
                    .on(world_lore_links::source_lore_entry_id.eq(world_lore_entries::id)),
            )
            .select(LoreEntry::as_select())
            .load::<LoreEntry>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load linked-from entries"))?;

    let rows = crate::moderation::filter_visible(state, "world_lore_entry", rows, |e| e.id).await?;
    Ok(rows.into_iter().map(GraphQLLoreEntry::from).collect())
}

/// Testable core of `LoreQuery::world_lore_entries`. Listing is not
/// permission-gated beyond world membership — `myPermissionLevel` on
/// each entry tells the client what UI to show (contracts/lore-crud.md).
pub async fn world_lore_entries_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<Vec<LoreEntry>> {
    require_visible_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let rows = tokio::task::spawn_blocking(move || {
        world_lore_entries::table
            .filter(world_lore_entries::world_id.eq(world_id))
            .select(LoreEntry::as_select())
            .load::<LoreEntry>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load world lore entries"))?;

    // Spec 015: excluded from list queries entirely when disabled.
    crate::moderation::filter_visible(state, "world_lore_entry", rows, |e| e.id).await
}

/// Testable core of `LoreQuery::lore_entry`. Returns `None` (not an
/// error) for a stale/nonexistent slug (FR-014); denies non-members
/// entirely (FR-015).
pub async fn lore_entry_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    slug: &str,
) -> GraphQLResult<Option<LoreEntry>> {
    require_visible_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let slug = slug.to_string();
    let row = tokio::task::spawn_blocking(move || {
        world_lore_entries::table
            .filter(world_lore_entries::world_id.eq(world_id))
            .filter(world_lore_entries::slug.eq(&slug))
            .select(LoreEntry::as_select())
            .first::<LoreEntry>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load lore entry"))?;

    // Spec 015: a single-entity query returns a moderation placeholder
    // instead of real content, for every caller including the owner
    // (contracts/graphql-moderation.md) — never excluded like a list.
    let Some(row) = row else {
        return Ok(None);
    };
    if crate::moderation::effective_status(state, "world_lore_entry", row.id)
        .await?
        .is_some()
    {
        let now = chrono::Utc::now().naive_utc();
        return Ok(Some(crate::models::LoreEntry {
            id: row.id,
            world_id: row.world_id,
            title: "[Content removed in response to a takedown notice]".to_string(),
            slug: row.slug,
            content: String::new(),
            current_revision_id: None,
            created_by: row.created_by,
            created_at: now,
            updated_at: now,
        }));
    }
    Ok(Some(row))
}

#[derive(Enum, Debug, Copy, Clone, Eq, PartialEq)]
pub enum GraphQLLoreLinkTargetKind {
    LoreEntry,
    Actor,
}

/// One autocomplete candidate for the editor's `[[`-trigger popover
/// (T034, FR-007a).
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLLoreLinkTarget {
    pub id: Uuid,
    pub title: String,
    pub kind: GraphQLLoreLinkTargetKind,
}

/// Testable core of `LoreQuery::lore_link_targets`. Returns lore entries
/// and actors whose title/label starts with `prefix` (case-insensitive),
/// distinct and disambiguated by `kind` (FR-007a) so an author picks the
/// intended target explicitly rather than typing an ambiguous title.
pub async fn lore_link_targets_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    prefix: &str,
) -> GraphQLResult<Vec<GraphQLLoreLinkTarget>> {
    require_visible_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let pattern = format!("{}%", prefix.replace('%', "\\%").replace('_', "\\_"));

    tokio::task::spawn_blocking(move || {
        let mut results = Vec::new();

        let lore_matches = world_lore_entries::table
            .filter(world_lore_entries::world_id.eq(world_id))
            .filter(world_lore_entries::title.ilike(&pattern))
            .select((world_lore_entries::id, world_lore_entries::title))
            .load::<(Uuid, String)>(&mut conn)?;
        results.extend(lore_matches.into_iter().map(|(id, title)| GraphQLLoreLinkTarget {
            id,
            title,
            kind: GraphQLLoreLinkTargetKind::LoreEntry,
        }));

        let actor_matches = world_actors::table
            .filter(world_actors::world_id.eq(world_id))
            .filter(world_actors::label.ilike(&pattern))
            .select((world_actors::id, world_actors::label))
            .load::<(Uuid, String)>(&mut conn)?;
        results.extend(actor_matches.into_iter().map(|(id, title)| GraphQLLoreLinkTarget {
            id,
            title,
            kind: GraphQLLoreLinkTargetKind::Actor,
        }));

        Ok::<_, diesel::result::Error>(results)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to search lore link targets"))
}

/// Testable core of `LoreQuery::lore_entry_revisions`. Viewer-or-above
/// check (FR-017); newest-first ordering.
///
/// `require_lore_permission`'s `Viewer` minimum alone is not a real gate
/// (every caller — member or not — defaults to `Viewer` when no explicit
/// row exists), so world membership is checked explicitly first, the
/// same way `world_lore_entries_impl`/`lore_entry_impl` do (FR-015).
pub async fn lore_entry_revisions_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    lore_entry_id: Uuid,
) -> GraphQLResult<Vec<LoreRevision>> {
    let world_id = {
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        tokio::task::spawn_blocking(move || {
            world_lore_entries::table
                .filter(world_lore_entries::id.eq(lore_entry_id))
                .select(world_lore_entries::world_id)
                .first::<Uuid>(&mut conn)
                .optional()
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load lore entry"))?
        .ok_or_else(|| Error::new("Lore entry not found"))?
    };
    require_visible_world(state, user_id, is_admin, world_id).await?;
    require_lore_permission(state, user_id, is_admin, lore_entry_id, ActorPermissionLevel::Viewer).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_lore_revisions::table
            .filter(world_lore_revisions::lore_entry_id.eq(lore_entry_id))
            .order(world_lore_revisions::created_at.desc())
            .select(LoreRevision::as_select())
            .load::<LoreRevision>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load lore entry revisions"))
}

#[derive(Default)]
pub struct LoreQuery;

#[async_graphql::Object]
impl LoreQuery {
    /// Every lore entry in the world, visible to any member (FR-001).
    async fn world_lore_entries(&self, ctx: &Context<'_>, world_id: Uuid) -> GraphQLResult<Vec<GraphQLLoreEntry>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let entries = world_lore_entries_impl(state, auth_user.user_id, auth_user.is_admin, world_id).await?;
        Ok(entries.into_iter().map(GraphQLLoreEntry::from).collect())
    }

    /// The canonical detail-page lookup by `(worldId, slug)` — `null` for
    /// a stale/nonexistent slug (FR-014).
    async fn lore_entry(&self, ctx: &Context<'_>, world_id: Uuid, slug: String) -> GraphQLResult<Option<GraphQLLoreEntry>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let entry = lore_entry_impl(state, auth_user.user_id, auth_user.is_admin, world_id, &slug).await?;
        Ok(entry.map(GraphQLLoreEntry::from))
    }

    /// Autocomplete candidates for the editor's `[[`-trigger popover
    /// (FR-007a).
    async fn lore_link_targets(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        prefix: String,
    ) -> GraphQLResult<Vec<GraphQLLoreLinkTarget>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        lore_link_targets_impl(state, auth_user.user_id, auth_user.is_admin, world_id, &prefix).await
    }

    /// Full revision history for a lore entry, newest first (FR-017).
    async fn lore_entry_revisions(&self, ctx: &Context<'_>, lore_entry_id: Uuid) -> GraphQLResult<Vec<GraphQLLoreRevision>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let revisions =
            lore_entry_revisions_impl(state, auth_user.user_id, auth_user.is_admin, lore_entry_id).await?;
        Ok(revisions.into_iter().map(GraphQLLoreRevision::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{insert_test_user, insert_test_world, insert_test_world_member, test_app_state};

    fn insert_lore_entry(conn: &mut diesel::PgConnection, world_id: Uuid, created_by: Uuid, title: &str, slug: &str) -> Uuid {
        let id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_lore_entries::table)
            .values((
                world_lore_entries::id.eq(id),
                world_lore_entries::world_id.eq(world_id),
                world_lore_entries::title.eq(title),
                world_lore_entries::slug.eq(slug),
                world_lore_entries::content.eq(""),
                world_lore_entries::created_by.eq(created_by),
                world_lore_entries::created_at.eq(now),
                world_lore_entries::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("failed to insert test lore entry");
        id
    }

    /// FR-001: any world member sees the full roster, unfiltered by
    /// per-entry permission.
    #[tokio::test]
    async fn world_lore_entries_returns_all_entries_for_a_member() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_lore_entry(&mut conn, world_id, owner_id, "Entry A", "entry-a");
        insert_lore_entry(&mut conn, world_id, owner_id, "Entry B", "entry-b");
        drop(conn);

        let entries = world_lore_entries_impl(&state, owner_id, false, world_id)
            .await
            .expect("owner should list entries");
        assert_eq!(entries.len(), 2);
    }

    /// FR-015: a non-member is denied entirely.
    #[tokio::test]
    async fn world_lore_entries_rejects_non_member() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let outsider_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let result = world_lore_entries_impl(&state, outsider_id, false, world_id).await;
        assert!(result.is_err(), "a non-member must not see the world's lore entries");
    }

    /// FR-014: `lore_entry` returns `None` (not an error) for a
    /// nonexistent slug.
    #[tokio::test]
    async fn lore_entry_returns_none_for_missing_slug() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let entry = lore_entry_impl(&state, owner_id, false, world_id, "does-not-exist")
            .await
            .expect("lookup should not error for a missing slug");
        assert!(entry.is_none());
    }

    /// FR-007a: `lore_link_targets` returns both a lore entry and an
    /// actor of the same title as distinct candidates.
    #[tokio::test]
    async fn lore_link_targets_disambiguates_same_title_across_kinds() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = crate::test_support::insert_test_scene(&mut conn, world_id, owner_id);
        insert_lore_entry(&mut conn, world_id, owner_id, "Ambiguous", "ambiguous");
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_actors::table)
            .values((
                world_actors::id.eq(Uuid::now_v7()),
                world_actors::world_id.eq(world_id),
                world_actors::scene_id.eq(scene_id),
                world_actors::actor_type.eq("npc"),
                world_actors::game_system_id.eq("generic"),
                world_actors::label.eq("Ambiguous"),
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

        let targets = lore_link_targets_impl(&state, owner_id, false, world_id, "Ambig")
            .await
            .expect("search should succeed");
        assert_eq!(targets.len(), 2);
        assert!(targets.iter().any(|t| t.kind == GraphQLLoreLinkTargetKind::LoreEntry));
        assert!(targets.iter().any(|t| t.kind == GraphQLLoreLinkTargetKind::Actor));
    }

    /// FR-017: revision history is Viewer-or-above gated and newest first.
    #[tokio::test]
    async fn lore_entry_revisions_requires_at_least_viewer() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let entry_id = insert_lore_entry(&mut conn, world_id, owner_id, "Entry A", "entry-a");
        let outsider_id = insert_test_user(&mut conn);
        drop(conn);

        let denied = lore_entry_revisions_impl(&state, outsider_id, false, entry_id).await;
        assert!(denied.is_err(), "a non-member/non-viewer must be denied");

        let mut conn = state.db_pool.get().unwrap();
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let allowed = lore_entry_revisions_impl(&state, player_id, false, entry_id).await;
        assert!(allowed.is_ok(), "a default-Viewer world member should be able to view history");
    }
}
