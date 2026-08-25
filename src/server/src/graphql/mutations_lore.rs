//! Spec 012: lore entry creation/editing/deletion/restore mutations
//! (`createLoreEntry`, `updateLoreEntry`, `deleteLoreEntry`,
//! `restoreLoreRevision`). See `specs/012-lore-wiki/contracts/lore-crud.md`,
//! `lore-revisions.md`.

use async_graphql::{Context, Error, ErrorExtensions, InputObject, Result as GraphQLResult};
use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::lore_permissions::{is_dm_of_world, require_lore_permission};
use crate::graphql::types::{ActorPermissionLevel, GraphQLLoreEntry};
use crate::graphql::{app_state, authenticated_user};
use crate::markdown::links::PreparedLink;
use crate::models::{LoreEntry, NewLoreLink, NewLoreRevision};
use crate::schema::{world_lore_entries, world_lore_links, world_lore_revisions};
use crate::state::AppState;

/// FR-010a: fixed default for this pass — see spec.md's Clarifications
/// and Assumptions (instance-configurable quotas are explicitly deferred
/// future work, not built here).
pub const MAX_LORE_CONTENT_BYTES: usize = 25 * 1024 * 1024;

#[derive(InputObject, Debug, Clone)]
pub struct CreateLoreEntryInput {
    pub world_id: Uuid,
    pub title: String,
    pub content: Option<String>,
}

#[derive(InputObject, Debug, Clone)]
pub struct UpdateLoreEntryInput {
    pub lore_entry_id: Uuid,
    pub title: Option<String>,
    pub content: Option<String>,
    /// REQUIRED whenever `content` is provided (FR-019) — the revision
    /// the author was editing against, compared to the entry's actual
    /// `current_revision_id` at write time to detect a conflicting
    /// concurrent save.
    pub expected_current_revision_id: Option<Uuid>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoreWriteError {
    #[error("database error: {0}")]
    Database(String),
    #[error("lore entry not found")]
    NotFound,
    #[error("lore entry content exceeds the 25 MB limit")]
    TooLarge,
    #[error("someone else saved this entry first; reload the current content and try again")]
    Conflict,
}

impl From<diesel::result::Error> for LoreWriteError {
    fn from(e: diesel::result::Error) -> Self {
        LoreWriteError::Database(e.to_string())
    }
}

fn to_graphql_error(e: LoreWriteError) -> Error {
    let msg = e.to_string();
    match e {
        LoreWriteError::Conflict => Error::new(msg).extend_with(|_, ext| ext.set("code", "CONFLICT")),
        LoreWriteError::NotFound => Error::new(msg).extend_with(|_, ext| ext.set("code", "NOT_FOUND")),
        _ => Error::new(msg),
    }
}

/// Deletes `source_lore_entry_id`'s existing outgoing `world_lore_links`
/// rows and inserts the freshly resolved set (research.md §2 — replaced
/// wholesale, never incrementally diffed).
fn replace_lore_links(
    conn: &mut PgConnection,
    source_lore_entry_id: Uuid,
    links: &[PreparedLink],
) -> Result<(), diesel::result::Error> {
    diesel::delete(
        world_lore_links::table.filter(world_lore_links::source_lore_entry_id.eq(source_lore_entry_id)),
    )
    .execute(conn)?;

    for link in links {
        diesel::insert_into(world_lore_links::table)
            .values(NewLoreLink {
                id: Uuid::now_v7(),
                source_lore_entry_id,
                raw_title: link.raw_title.clone(),
                target_kind: link.target_kind.to_string(),
                target_lore_entry_id: link.target_lore_entry_id,
                target_actor_id: link.target_actor_id,
                target_item_id: link.target_item_id,
                target_ability_id: link.target_ability_id,
            })
            .execute(conn)?;
    }

    Ok(())
}

/// Testable core of `LoreMutation::create_lore_entry`. DM-only (FR-002).
pub async fn create_lore_entry_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: CreateLoreEntryInput,
) -> Result<LoreEntry, LoreWriteError> {
    if !is_dm_of_world(state, user_id, is_admin, input.world_id)
        .await
        .map_err(|e| LoreWriteError::Database(e.message))?
    {
        return Err(LoreWriteError::Database(
            "Only the DM (Owner or GM) may create lore entries".to_string(),
        ));
    }

    let content = input.content.clone().unwrap_or_default();
    if content.len() > MAX_LORE_CONTENT_BYTES {
        return Err(LoreWriteError::TooLarge);
    }

    let world_id = input.world_id;
    let title = input.title.clone();
    let mut conn = state
        .db_pool
        .get()
        .map_err(|e| LoreWriteError::Database(e.to_string()))?;

    tokio::task::spawn_blocking(move || {
        conn.transaction::<LoreEntry, LoreWriteError, _>(|conn| {
            let slug = crate::markdown::slug::unique_slug_for_world(conn, world_id, &title, None)?;
            let entry_id = Uuid::now_v7();
            let now = Utc::now().naive_utc();

            diesel::insert_into(world_lore_entries::table)
                .values((
                    world_lore_entries::id.eq(entry_id),
                    world_lore_entries::world_id.eq(world_id),
                    world_lore_entries::title.eq(&title),
                    world_lore_entries::slug.eq(&slug),
                    world_lore_entries::content.eq(&content),
                    world_lore_entries::created_by.eq(user_id),
                    world_lore_entries::created_at.eq(now),
                    world_lore_entries::updated_at.eq(now),
                ))
                .execute(conn)?;

            if !content.is_empty() {
                let revision_id = Uuid::now_v7();
                diesel::insert_into(world_lore_revisions::table)
                    .values(NewLoreRevision {
                        id: revision_id,
                        lore_entry_id: entry_id,
                        content_markdown: content.clone(),
                        author_id: user_id,
                        restored_from_revision_id: None,
                        created_at: now,
                    })
                    .execute(conn)?;

                diesel::update(world_lore_entries::table.filter(world_lore_entries::id.eq(entry_id)))
                    .set(world_lore_entries::current_revision_id.eq(revision_id))
                    .execute(conn)?;

                let (_, links) = crate::markdown::links::extract_and_resolve(conn, world_id, &content, true)?;
                replace_lore_links(conn, entry_id, &links)?;
            }

            world_lore_entries::table
                .filter(world_lore_entries::id.eq(entry_id))
                .select(LoreEntry::as_select())
                .first::<LoreEntry>(conn)
                .map_err(LoreWriteError::from)
        })
    })
    .await
    .map_err(|_| LoreWriteError::Database("Failed to spawn blocking task".to_string()))?
}

/// Testable core of `LoreMutation::update_lore_entry`. Editor/Owner
/// permission (FR-003); regenerates slug on title change (FR-014);
/// enforces the 25 MB content cap (FR-010a) and optimistic-concurrency
/// conflict detection (FR-019) before any row is written.
pub async fn update_lore_entry_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: UpdateLoreEntryInput,
) -> Result<LoreEntry, LoreWriteError> {
    require_lore_permission(
        state,
        user_id,
        is_admin,
        input.lore_entry_id,
        ActorPermissionLevel::Editor,
    )
    .await
    .map_err(|e| LoreWriteError::Database(e.message))?;

    if let Some(content) = &input.content
        && content.len() > MAX_LORE_CONTENT_BYTES
    {
        return Err(LoreWriteError::TooLarge);
    }

    let entry_id = input.lore_entry_id;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|e| LoreWriteError::Database(e.to_string()))?;

    tokio::task::spawn_blocking(move || {
        conn.transaction::<LoreEntry, LoreWriteError, _>(|conn| {
            let existing = world_lore_entries::table
                .filter(world_lore_entries::id.eq(entry_id))
                .select(LoreEntry::as_select())
                .first::<LoreEntry>(conn)
                .optional()?
                .ok_or(LoreWriteError::NotFound)?;

            let now = Utc::now().naive_utc();
            let new_title = input.title.clone().unwrap_or_else(|| existing.title.clone());
            let new_slug = if let Some(title) = &input.title
                && title != &existing.title
            {
                crate::markdown::slug::unique_slug_for_world(conn, existing.world_id, title, Some(entry_id))?
            } else {
                existing.slug.clone()
            };

            let mut current_revision_id = existing.current_revision_id;
            let mut new_content = existing.content.clone();

            if let Some(content) = &input.content {
                if existing.current_revision_id != input.expected_current_revision_id {
                    return Err(LoreWriteError::Conflict);
                }

                let revision_id = Uuid::now_v7();
                diesel::insert_into(world_lore_revisions::table)
                    .values(NewLoreRevision {
                        id: revision_id,
                        lore_entry_id: entry_id,
                        content_markdown: content.clone(),
                        author_id: user_id,
                        restored_from_revision_id: None,
                        created_at: now,
                    })
                    .execute(conn)?;

                let (_, links) =
                    crate::markdown::links::extract_and_resolve(conn, existing.world_id, content, true)?;
                replace_lore_links(conn, entry_id, &links)?;

                current_revision_id = Some(revision_id);
                new_content = content.clone();
            }

            diesel::update(world_lore_entries::table.filter(world_lore_entries::id.eq(entry_id)))
                .set((
                    world_lore_entries::title.eq(&new_title),
                    world_lore_entries::slug.eq(&new_slug),
                    world_lore_entries::content.eq(&new_content),
                    world_lore_entries::current_revision_id.eq(current_revision_id),
                    world_lore_entries::updated_at.eq(now),
                ))
                .execute(conn)?;

            world_lore_entries::table
                .filter(world_lore_entries::id.eq(entry_id))
                .select(LoreEntry::as_select())
                .first::<LoreEntry>(conn)
                .map_err(LoreWriteError::from)
        })
    })
    .await
    .map_err(|_| LoreWriteError::Database("Failed to spawn blocking task".to_string()))?
}

/// Testable core of `LoreMutation::delete_lore_entry`. Owner-level
/// permission (FR-021, entry-level Owner per spec Clarifications, not
/// DM-only). Cascades (revisions/permissions/images/outgoing links) and
/// nulls out other entries'/actors' incoming links are both handled by
/// the migrations' `ON DELETE` actions — no application-level cleanup
/// needed (FR-020).
pub async fn delete_lore_entry_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    lore_entry_id: Uuid,
) -> Result<bool, LoreWriteError> {
    require_lore_permission(state, user_id, is_admin, lore_entry_id, ActorPermissionLevel::Owner)
        .await
        .map_err(|e| LoreWriteError::Database(e.message))?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|e| LoreWriteError::Database(e.to_string()))?;

    let deleted = tokio::task::spawn_blocking(move || {
        diesel::delete(world_lore_entries::table.filter(world_lore_entries::id.eq(lore_entry_id))).execute(&mut conn)
    })
    .await
    .map_err(|_| LoreWriteError::Database("Failed to spawn blocking task".to_string()))?
    .map_err(LoreWriteError::from)?;

    Ok(deleted > 0)
}

/// Testable core of `LoreMutation::restore_lore_revision`. Editor/Owner
/// on the revision's parent entry (FR-018); appends a new revision
/// recording the restore rather than deleting/mutating any existing one.
pub async fn restore_lore_revision_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    revision_id: Uuid,
) -> Result<LoreEntry, LoreWriteError> {
    let mut lookup_conn = state
        .db_pool
        .get()
        .map_err(|e| LoreWriteError::Database(e.to_string()))?;

    let lore_entry_id = tokio::task::spawn_blocking(move || {
        world_lore_revisions::table
            .filter(world_lore_revisions::id.eq(revision_id))
            .select(world_lore_revisions::lore_entry_id)
            .first::<Uuid>(&mut lookup_conn)
            .optional()
    })
    .await
    .map_err(|_| LoreWriteError::Database("Failed to spawn blocking task".to_string()))?
    .map_err(LoreWriteError::from)?
    .ok_or(LoreWriteError::NotFound)?;

    require_lore_permission(state, user_id, is_admin, lore_entry_id, ActorPermissionLevel::Editor)
        .await
        .map_err(|e| LoreWriteError::Database(e.message))?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|e| LoreWriteError::Database(e.to_string()))?;

    tokio::task::spawn_blocking(move || {
        conn.transaction::<LoreEntry, LoreWriteError, _>(|conn| {
            let target = world_lore_revisions::table
                .filter(world_lore_revisions::id.eq(revision_id))
                .select(crate::models::LoreRevision::as_select())
                .first::<crate::models::LoreRevision>(conn)?;

            let entry = world_lore_entries::table
                .filter(world_lore_entries::id.eq(lore_entry_id))
                .select(LoreEntry::as_select())
                .first::<LoreEntry>(conn)?;

            let now = Utc::now().naive_utc();
            let new_revision_id = Uuid::now_v7();
            diesel::insert_into(world_lore_revisions::table)
                .values(NewLoreRevision {
                    id: new_revision_id,
                    lore_entry_id,
                    content_markdown: target.content_markdown.clone(),
                    author_id: user_id,
                    restored_from_revision_id: Some(target.id),
                    created_at: now,
                })
                .execute(conn)?;

            let (_, links) =
                crate::markdown::links::extract_and_resolve(conn, entry.world_id, &target.content_markdown, true)?;
            replace_lore_links(conn, lore_entry_id, &links)?;

            diesel::update(world_lore_entries::table.filter(world_lore_entries::id.eq(lore_entry_id)))
                .set((
                    world_lore_entries::content.eq(&target.content_markdown),
                    world_lore_entries::current_revision_id.eq(new_revision_id),
                    world_lore_entries::updated_at.eq(now),
                ))
                .execute(conn)?;

            world_lore_entries::table
                .filter(world_lore_entries::id.eq(lore_entry_id))
                .select(LoreEntry::as_select())
                .first::<LoreEntry>(conn)
                .map_err(LoreWriteError::from)
        })
    })
    .await
    .map_err(|_| LoreWriteError::Database("Failed to spawn blocking task".to_string()))?
}

#[derive(Default)]
pub struct LoreMutation;

#[async_graphql::Object]
impl LoreMutation {
    async fn create_lore_entry(&self, ctx: &Context<'_>, input: CreateLoreEntryInput) -> GraphQLResult<GraphQLLoreEntry> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        create_lore_entry_impl(state, auth_user.user_id, auth_user.is_admin, input)
            .await
            .map(GraphQLLoreEntry::from)
            .map_err(to_graphql_error)
    }

    async fn update_lore_entry(&self, ctx: &Context<'_>, input: UpdateLoreEntryInput) -> GraphQLResult<GraphQLLoreEntry> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        update_lore_entry_impl(state, auth_user.user_id, auth_user.is_admin, input)
            .await
            .map(GraphQLLoreEntry::from)
            .map_err(to_graphql_error)
    }

    async fn delete_lore_entry(&self, ctx: &Context<'_>, lore_entry_id: Uuid) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        delete_lore_entry_impl(state, auth_user.user_id, auth_user.is_admin, lore_entry_id)
            .await
            .map_err(to_graphql_error)
    }

    async fn restore_lore_revision(&self, ctx: &Context<'_>, revision_id: Uuid) -> GraphQLResult<GraphQLLoreEntry> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        restore_lore_revision_impl(state, auth_user.user_id, auth_user.is_admin, revision_id)
            .await
            .map(GraphQLLoreEntry::from)
            .map_err(to_graphql_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{insert_test_user, insert_test_world, insert_test_world_member, test_app_state};

    /// FR-002: only the DM may create a lore entry; FR-012/FR-013: slug
    /// is derived from the title.
    #[tokio::test]
    async fn dm_can_create_lore_entry_with_derived_slug() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let entry = create_lore_entry_impl(
            &state,
            owner_id,
            false,
            CreateLoreEntryInput {
                world_id,
                title: "Ancient Ruins of Veldrath".to_string(),
                content: None,
            },
        )
        .await
        .expect("DM should be able to create a lore entry");

        assert_eq!(entry.title, "Ancient Ruins of Veldrath");
        assert_eq!(entry.slug, "ancient-ruins-of-veldrath");
        assert!(entry.current_revision_id.is_none(), "empty initial content creates no revision");
    }

    /// FR-002: a non-DM caller is rejected.
    #[tokio::test]
    async fn non_dm_cannot_create_lore_entry() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let result = create_lore_entry_impl(
            &state,
            player_id,
            false,
            CreateLoreEntryInput {
                world_id,
                title: "Should not exist".to_string(),
                content: None,
            },
        )
        .await;
        assert!(result.is_err(), "a Player-role caller must not be able to create lore entries");
    }

    /// FR-016: a content-bearing update appends a revision and updates
    /// `current_revision_id`.
    #[tokio::test]
    async fn update_with_content_appends_revision() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let entry = create_lore_entry_impl(
            &state,
            owner_id,
            false,
            CreateLoreEntryInput { world_id, title: "Entry".to_string(), content: None },
        )
        .await
        .unwrap();
        assert!(entry.current_revision_id.is_none());

        let updated = update_lore_entry_impl(
            &state,
            owner_id,
            false,
            UpdateLoreEntryInput {
                lore_entry_id: entry.id,
                title: None,
                content: Some("Hello world".to_string()),
                expected_current_revision_id: None,
            },
        )
        .await
        .expect("first content save should succeed (expected=None matches entry's None)");
        assert!(updated.current_revision_id.is_some());
        assert_eq!(updated.content, "Hello world");
    }

    /// FR-019: a save whose `expectedCurrentRevisionId` no longer matches
    /// the entry's actual latest revision is rejected outright with a
    /// conflict — never silently overwritten.
    #[tokio::test]
    async fn concurrent_conflicting_save_is_rejected() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let entry = create_lore_entry_impl(
            &state,
            owner_id,
            false,
            CreateLoreEntryInput { world_id, title: "Entry".to_string(), content: Some("v1".to_string()) },
        )
        .await
        .unwrap();
        let v1_revision = entry.current_revision_id;

        // First author saves successfully against v1.
        let v2 = update_lore_entry_impl(
            &state,
            owner_id,
            false,
            UpdateLoreEntryInput {
                lore_entry_id: entry.id,
                title: None,
                content: Some("v2".to_string()),
                expected_current_revision_id: v1_revision,
            },
        )
        .await
        .expect("first save against the correct expected revision should succeed");
        assert_eq!(v2.content, "v2");

        // Second author, still holding the stale v1 revision id, is rejected.
        let conflict = update_lore_entry_impl(
            &state,
            owner_id,
            false,
            UpdateLoreEntryInput {
                lore_entry_id: entry.id,
                title: None,
                content: Some("conflicting v2".to_string()),
                expected_current_revision_id: v1_revision,
            },
        )
        .await;
        assert!(matches!(conflict, Err(LoreWriteError::Conflict)));

        // The rejected save must not have overwritten anything.
        let mut conn = state.db_pool.get().unwrap();
        let reloaded = world_lore_entries::table
            .filter(world_lore_entries::id.eq(entry.id))
            .select(LoreEntry::as_select())
            .first::<LoreEntry>(&mut conn)
            .unwrap();
        assert_eq!(reloaded.content, "v2", "the rejected conflicting save must not appear");
    }

    /// FR-021: entry-level Owner (not DM-only) can delete; Editor cannot.
    #[tokio::test]
    async fn owner_level_can_delete_but_editor_cannot() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let entry = create_lore_entry_impl(
            &state,
            owner_id,
            false,
            CreateLoreEntryInput { world_id, title: "Entry".to_string(), content: None },
        )
        .await
        .unwrap();

        let mut conn = state.db_pool.get().unwrap();
        let editor_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, editor_id, "Player");
        diesel::insert_into(crate::schema::world_lore_permissions::table)
            .values((
                crate::schema::world_lore_permissions::id.eq(Uuid::now_v7()),
                crate::schema::world_lore_permissions::lore_entry_id.eq(entry.id),
                crate::schema::world_lore_permissions::world_member_user_id.eq(editor_id),
                crate::schema::world_lore_permissions::level.eq("Editor"),
            ))
            .execute(&mut conn)
            .unwrap();
        drop(conn);

        let denied = delete_lore_entry_impl(&state, editor_id, false, entry.id).await;
        assert!(denied.is_err(), "an Editor-level (not Owner-level) member must not be able to delete");

        let allowed = delete_lore_entry_impl(&state, owner_id, false, entry.id).await;
        assert!(allowed.is_ok(), "the DM (implicit Owner) should be able to delete");
    }

    /// FR-018: restoring a prior revision appends a new revision (never
    /// deletes history) and updates the entry's current content.
    #[tokio::test]
    async fn restore_appends_new_revision_without_deleting_history() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let entry = create_lore_entry_impl(
            &state,
            owner_id,
            false,
            CreateLoreEntryInput { world_id, title: "Entry".to_string(), content: Some("v1".to_string()) },
        )
        .await
        .unwrap();
        let v1_revision = entry.current_revision_id.unwrap();

        let v2 = update_lore_entry_impl(
            &state,
            owner_id,
            false,
            UpdateLoreEntryInput {
                lore_entry_id: entry.id,
                title: None,
                content: Some("v2".to_string()),
                expected_current_revision_id: Some(v1_revision),
            },
        )
        .await
        .unwrap();
        assert_eq!(v2.content, "v2");

        let restored = restore_lore_revision_impl(&state, owner_id, false, v1_revision)
            .await
            .expect("restoring an earlier revision should succeed");
        assert_eq!(restored.content, "v1");

        let mut conn = state.db_pool.get().unwrap();
        let revision_count: i64 = world_lore_revisions::table
            .filter(world_lore_revisions::lore_entry_id.eq(entry.id))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(revision_count, 3, "v1, v2, and the restore-as-v3 must all remain in history");
    }
}
