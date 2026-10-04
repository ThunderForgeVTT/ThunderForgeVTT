//! The world lore wiki: entries, their revisions, images and permissions
//! (spec 012).

use super::ActorPermissionLevel;
use async_graphql::SimpleObject;
use chrono::NaiveDateTime;

use crate::models::{LoreEntry, LoreImageAsset, LorePermission, LoreRevision};

/// A world-scoped wiki page (FR-001..FR-021). `myPermissionLevel` and
/// `renderedHtml` are per-request-computed complex fields: `content` is
/// re-rendered (GFM parse + link resolution + sanitize) on every read
/// rather than cached, keeping `renderedHtml` always consistent with the
/// current `content`/live `world_lore_links` state (research.md 1, 2).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex)]
pub struct GraphQLLoreEntry {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub current_revision_id: Option<uuid::Uuid>,
    pub created_by: uuid::Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    /// Spec 031 (FR-038): the entry this one sits under, `null` for a root.
    /// A plain column rather than a nested `parent` object — the client
    /// already loads the world's entries to draw the tree, and an object
    /// here would be the same rows fetched twice.
    pub parent_id: Option<uuid::Uuid>,
    /// Spec 015: true when this entry is currently disabled in response to
    /// a DMCA takedown notice — `title`/`content` are a placeholder, not
    /// the real content (contracts/graphql-moderation.md).
    pub moderated: bool,
    /// The disabling case's id, present only on a moderation placeholder.
    pub moderation_case_id: Option<uuid::Uuid>,
}

#[async_graphql::ComplexObject]
impl GraphQLLoreEntry {
    /// Effective Viewer/Editor/Owner level the calling user holds on this
    /// entry: DM of the entry's world always resolves to Owner;
    /// otherwise the caller's explicit `world_lore_permissions` row, else
    /// Viewer (FR-003).
    async fn my_permission_level(
        &self,
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<ActorPermissionLevel> {
        let state = crate::graphql::app_state(ctx)?;
        let auth_user = crate::graphql::authenticated_user(ctx)?;
        crate::auth::lore_permissions::effective_lore_permission(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            self.id,
        )
        .await
    }

    /// Server-rendered, sanitized GFM HTML for `content`, with resolved
    /// in-text links substituted in as real anchors/broken-link spans
    /// (FR-004, FR-005, FR-007).
    async fn rendered_html(
        &self,
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<String> {
        crate::graphql::queries::lore::render_lore_content(ctx, self.world_id, &self.content).await
    }

    /// Every lore entry whose body currently contains a resolved in-text
    /// link to this entry (FR-006).
    async fn linked_from(
        &self,
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<Vec<GraphQLLoreEntry>> {
        crate::graphql::queries::lore::lore_entries_linking_to(ctx, self.id).await
    }

    /// Spec 031 (FR-038): this entry's tags, normalised and alphabetical.
    ///
    /// A resolver rather than a column, for the same reason `price` is one on
    /// an item: tags live in their own table, and a moderation placeholder
    /// must not carry any — a blanked entry still labelled "ancient ruins"
    /// would be the one piece of real content left on it.
    async fn tags(&self, ctx: &async_graphql::Context<'_>) -> async_graphql::Result<Vec<String>> {
        if self.moderated {
            return Ok(Vec::new());
        }
        let state = crate::graphql::app_state(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;
        let entry_id = self.id;
        tokio::task::spawn_blocking(move || {
            crate::graphql::mutations_lore_tree::tags_for_entry(&mut conn, entry_id)
        })
        .await
        .map_err(|_| async_graphql::Error::new("Failed to spawn blocking task"))?
        .map_err(|_| async_graphql::Error::new("Failed to load lore tags"))
    }
}

impl From<LoreEntry> for GraphQLLoreEntry {
    fn from(row: LoreEntry) -> Self {
        Self {
            id: row.id,
            world_id: row.world_id,
            title: row.title,
            slug: row.slug,
            content: row.content,
            current_revision_id: row.current_revision_id,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
            parent_id: row.parent_id,
            moderated: false,
            moderation_case_id: None,
        }
    }
}

/// An immutable snapshot of a lore entry's Markdown content at one point
/// in save time (FR-016/017/018).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex)]
pub struct GraphQLLoreRevision {
    pub id: uuid::Uuid,
    pub lore_entry_id: uuid::Uuid,
    pub content_markdown: String,
    pub author_id: uuid::Uuid,
    pub restored_from_revision_id: Option<uuid::Uuid>,
    pub created_at: NaiveDateTime,
}

#[async_graphql::ComplexObject]
impl GraphQLLoreRevision {
    /// Re-rendered on read for this specific historical revision
    /// (contracts/lore-revisions.md) - resolves in-text links against
    /// the world's current entries/actors (a past revision's links are
    /// not themselves versioned; only its Markdown text is).
    async fn rendered_html(
        &self,
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<String> {
        let world_id =
            crate::graphql::queries::lore::world_id_for_lore_entry(ctx, self.lore_entry_id).await?;
        crate::graphql::queries::lore::render_lore_content(ctx, world_id, &self.content_markdown)
            .await
    }
}

impl From<LoreRevision> for GraphQLLoreRevision {
    fn from(row: LoreRevision) -> Self {
        Self {
            id: row.id,
            lore_entry_id: row.lore_entry_id,
            content_markdown: row.content_markdown,
            author_id: row.author_id,
            restored_from_revision_id: row.restored_from_revision_id,
            created_at: row.created_at,
        }
    }
}

/// A lore entry's ownership-block entry: one explicit (lore entry,
/// world member, permission level) grant. Direct structural mirror of
/// `GraphQLActorPermission` (spec 010), generalized to lore entries.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLLorePermission {
    pub lore_entry_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub level: ActorPermissionLevel,
    pub updated_at: NaiveDateTime,
}

impl From<LorePermission> for GraphQLLorePermission {
    fn from(row: LorePermission) -> Self {
        Self {
            lore_entry_id: row.lore_entry_id,
            user_id: row.world_member_user_id,
            level: ActorPermissionLevel::from_db_str(&row.level)
                .unwrap_or(ActorPermissionLevel::Viewer),
            updated_at: row.updated_at,
        }
    }
}

/// An uploaded/pasted image attached to a lore entry (FR-008/009).
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLLoreImageAsset {
    pub id: uuid::Uuid,
    pub lore_entry_id: uuid::Uuid,
    pub url: String,
    pub thumbnail_url: String,
    pub byte_size: i32,
    pub created_at: NaiveDateTime,
}

impl From<LoreImageAsset> for GraphQLLoreImageAsset {
    fn from(row: LoreImageAsset) -> Self {
        Self {
            id: row.id,
            lore_entry_id: row.lore_entry_id,
            url: format!("/lore-assets/{}", row.id),
            thumbnail_url: format!("/lore-assets/{}/thumb", row.id),
            byte_size: row.byte_size as i32,
            created_at: row.created_at,
        }
    }
}

// ============================================================================
