//! Collections on the shelf, on the wire (spec 049 Phase 13, spec 050 FR-007
//! to FR-009c).
//!
//! Named "shelf collection" rather than "collection" because spec 026 already
//! owns that word on the wire, for a world's own gathering of its artifacts.
//! The two are different things with different lifetimes: that one lives in a
//! world, this one on an account's shelf beside the books read in.
//!
//! Every rule is [`crate::compendium::collections`]'s. What this module adds
//! is the one check a store cannot make without a manifest: that a collection
//! names a system which declares kinds of content, and that each entry is one
//! of those kinds — the check an import makes of what it read (FR-015).

use std::collections::BTreeMap;

use async_graphql::{Context, Error, Json, Object, SimpleObject};
use thunderforge_canvas_core::content_patterns::ContentPatterns;
use uuid::Uuid;

use crate::compendium::collections::{self, CollectionError};
use crate::compendium::versions::{self, At, PastVersion, VersionEntry};
use crate::content::ReadValue;
use crate::content_patterns::content_patterns_for_system;
use crate::graphql::mutations_library::content_from;
use crate::graphql::queries::compendium::{
    GraphQLCompendium, GraphQLCompendiumEntry, GraphQLKindCount, kind_counts, to_graphql_entry,
};
use crate::graphql::{GraphQLResult, app_state, authenticated_user};
use crate::state::AppState;

/// Something left out of a download, and why (FR-009c).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "ShelfCollectionExcludedEntry")]
pub struct GraphQLExcludedEntry {
    pub kind: String,
    pub name: String,
    pub reason: String,
}

/// A collection as a file its owner takes away (FR-009a).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "ShelfCollectionDownload")]
pub struct GraphQLShelfCollectionDownload {
    /// A name to save it under.
    pub file_name: String,
    /// The file itself: JSON, as it is saved.
    pub contents: String,
    pub entry_count: i32,
    /// What was left out, named, so a thinner file is never silent (FR-009c).
    pub excluded: Vec<GraphQLExcludedEntry>,
}

/// An earlier version of a collection, named without its content (050
/// FR-104).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "ShelfCollectionVersion")]
pub struct GraphQLShelfCollectionVersion {
    pub version: i32,
    pub book_title: String,
    pub entry_counts: Vec<GraphQLKindCount>,
    pub entry_total: i32,
    /// What moved the collection on from this version, e.g. "Synced from The
    /// Sunken Keep".
    pub replaced_by: String,
    pub replaced_at: String,
}

impl From<PastVersion> for GraphQLShelfCollectionVersion {
    fn from(past: PastVersion) -> Self {
        Self {
            version: past.version,
            book_title: past.book_title,
            entry_counts: kind_counts(&past.entry_counts),
            entry_total: past.entry_total,
            replaced_by: past.replaced_by,
            replaced_at: past.replaced_at.and_utc().to_rfc3339(),
        }
    }
}

/// One entry as a version of a collection holds it.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "ShelfCollectionVersionEntry")]
pub struct GraphQLVersionEntry {
    pub kind: String,
    pub name: String,
    pub field_values: Json<serde_json::Value>,
    pub prose_text: Option<String>,
}

impl From<VersionEntry> for GraphQLVersionEntry {
    fn from(entry: VersionEntry) -> Self {
        Self {
            kind: entry.kind,
            name: entry.name,
            field_values: Json(entry.field_values),
            prose_text: entry.prose_text,
        }
    }
}

fn refusal(e: CollectionError) -> Error {
    Error::new(e.to_string())
}

fn connection(
    state: &AppState,
) -> GraphQLResult<
    diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>>,
> {
    state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))
}

/// What a system declares, or the refusal an import would give.
fn declared_patterns(systems_dir: &str, system_id: &str) -> GraphQLResult<ContentPatterns> {
    let patterns = content_patterns_for_system(systems_dir, system_id);
    if patterns.is_empty() {
        return Err(Error::new(
            "This game system does not say what kinds of content it has, so a collection cannot \
             be kept for it. A system declares that in its own manifest.",
        ));
    }
    Ok(patterns)
}

/// Testable core of `createShelfCollection`.
pub async fn create_shelf_collection_impl(
    state: &AppState,
    systems_dir: &str,
    owner: Uuid,
    title: String,
    system_id: String,
) -> GraphQLResult<GraphQLCompendium> {
    declared_patterns(systems_dir, &system_id)?;
    let mut conn = connection(state)?;
    tokio::task::spawn_blocking(move || {
        collections::create_collection(&mut conn, owner, &title, &system_id)
            .map(GraphQLCompendium::from)
            .map_err(refusal)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Testable core of `writeShelfCollectionEntry`.
#[allow(clippy::too_many_arguments)]
pub async fn write_shelf_collection_entry_impl(
    state: &AppState,
    systems_dir: &str,
    caller: Uuid,
    collection_id: Uuid,
    kind: String,
    name: String,
    field_values: Option<Json<BTreeMap<String, ReadValue>>>,
    prose_text: Option<String>,
) -> GraphQLResult<GraphQLCompendiumEntry> {
    let content = content_from(field_values, prose_text)?;
    let systems_dir = systems_dir.to_string();
    let mut conn = connection(state)?;
    tokio::task::spawn_blocking(move || {
        // Ownership first, through the store, so a stranger learns nothing
        // about the collection's system from the kind check below.
        let collection = crate::compendium::store::load(&mut conn, caller, collection_id)
            .map_err(|e| Error::new(e.to_string()))?;
        if declared_patterns(&systems_dir, &collection.system_id)?
            .for_kind(&kind)
            .is_none()
        {
            return Err(Error::new(format!(
                "'{kind}' is not a kind of content {} declares.",
                collection.system_id
            )));
        }
        collections::write_entry(&mut conn, caller, collection_id, &kind, &name, content)
            .map(|entry| to_graphql_entry(&entry, &collection.book_title))
            .map_err(refusal)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Testable core of `removeShelfCollectionEntry`.
pub async fn remove_shelf_collection_entry_impl(
    state: &AppState,
    caller: Uuid,
    collection_id: Uuid,
    entry_id: Uuid,
) -> GraphQLResult<bool> {
    let mut conn = connection(state)?;
    tokio::task::spawn_blocking(move || {
        collections::remove_entry(&mut conn, caller, collection_id, entry_id)
            .map(|()| true)
            .map_err(refusal)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Testable core of `downloadShelfCollection`.
pub async fn download_shelf_collection_impl(
    state: &AppState,
    caller: Uuid,
    collection_id: Uuid,
) -> GraphQLResult<GraphQLShelfCollectionDownload> {
    let mut conn = connection(state)?;
    let file = tokio::task::spawn_blocking(move || {
        collections::download(&mut conn, caller, collection_id).map_err(refusal)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))??;

    let contents = serde_json::to_string_pretty(&file)
        .map_err(|e| Error::new(format!("The collection could not be written out: {e}")))?;
    Ok(GraphQLShelfCollectionDownload {
        file_name: file_name_for(&file.title),
        contents,
        entry_count: i32::try_from(file.entries.len()).unwrap_or(i32::MAX),
        excluded: file
            .excluded
            .into_iter()
            .map(|excluded| GraphQLExcludedEntry {
                kind: excluded.kind,
                name: excluded.name,
                reason: excluded.reason,
            })
            .collect(),
    })
}

/// Testable core of `shelfCollectionVersions`.
pub async fn shelf_collection_versions_impl(
    state: &AppState,
    caller: Uuid,
    collection_id: Uuid,
) -> GraphQLResult<Vec<GraphQLShelfCollectionVersion>> {
    let mut conn = connection(state)?;
    tokio::task::spawn_blocking(move || {
        versions::history(&mut conn, caller, collection_id)
            .map(|history| history.into_iter().map(Into::into).collect())
            .map_err(refusal)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Testable core of `shelfCollectionVersionEntries`.
pub async fn shelf_collection_version_entries_impl(
    state: &AppState,
    caller: Uuid,
    collection_id: Uuid,
    version: i32,
) -> GraphQLResult<Vec<GraphQLVersionEntry>> {
    let mut conn = connection(state)?;
    tokio::task::spawn_blocking(move || {
        versions::read_at(&mut conn, caller, collection_id, At::Version(version))
            .map(|entries| entries.into_iter().map(Into::into).collect())
            .map_err(refusal)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Testable core of `restoreShelfCollectionVersion`.
pub async fn restore_shelf_collection_version_impl(
    state: &AppState,
    caller: Uuid,
    collection_id: Uuid,
    version: i32,
) -> GraphQLResult<GraphQLCompendium> {
    let mut conn = connection(state)?;
    tokio::task::spawn_blocking(move || {
        versions::restore(&mut conn, caller, collection_id, version)
            .map(GraphQLCompendium::from)
            .map_err(refusal)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// A file name a person recognises, safe on every file system they might save
/// it to.
fn file_name_for(title: &str) -> String {
    let stem: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim()
        .to_string();
    let stem = if stem.is_empty() { "collection" } else { &stem };
    format!("{stem}.json")
}

#[derive(Default)]
pub struct ShelfCollectionMutation;

#[Object]
impl ShelfCollectionMutation {
    /// Start a collection on the caller's shelf, for one game system (050
    /// FR-007, FR-009).
    async fn create_shelf_collection(
        &self,
        ctx: &Context<'_>,
        title: String,
        system_id: String,
    ) -> GraphQLResult<GraphQLCompendium> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        let systems_dir = state.directories.systems_dir.clone();
        create_shelf_collection_impl(state, &systems_dir, user.user_id, title, system_id).await
    }

    /// Write an entry into one of the caller's collections: `fieldValues` or
    /// `proseText`, not both. Refused for a book read in, whose entries are
    /// what the book says.
    async fn write_shelf_collection_entry(
        &self,
        ctx: &Context<'_>,
        collection_id: Uuid,
        kind: String,
        name: String,
        field_values: Option<Json<BTreeMap<String, ReadValue>>>,
        prose_text: Option<String>,
    ) -> GraphQLResult<GraphQLCompendiumEntry> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        let systems_dir = state.directories.systems_dir.clone();
        write_shelf_collection_entry_impl(
            state,
            &systems_dir,
            user.user_id,
            collection_id,
            kind,
            name,
            field_values,
            prose_text,
        )
        .await
    }

    /// Take an entry out of one of the caller's collections.
    async fn remove_shelf_collection_entry(
        &self,
        ctx: &Context<'_>,
        collection_id: Uuid,
        entry_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        remove_shelf_collection_entry_impl(state, user.user_id, collection_id, entry_id).await
    }

    /// Put an earlier version of one of the caller's collections back, as a
    /// new version; the one it replaces is kept too (050 FR-104).
    async fn restore_shelf_collection_version(
        &self,
        ctx: &Context<'_>,
        collection_id: Uuid,
        version: i32,
    ) -> GraphQLResult<GraphQLCompendium> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        restore_shelf_collection_version_impl(state, user.user_id, collection_id, version).await
    }
}

#[derive(Default)]
pub struct ShelfCollectionQuery;

#[Object]
impl ShelfCollectionQuery {
    /// One of the caller's collections as a JSON file (050 FR-009a). A book
    /// read in is refused (FR-009b), and anything left out is named
    /// (FR-009c).
    async fn download_shelf_collection(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> GraphQLResult<GraphQLShelfCollectionDownload> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        download_shelf_collection_impl(state, user.user_id, id).await
    }

    /// One of the caller's collections' earlier versions, newest first (050
    /// FR-104). Its owner's alone (ADR-098).
    async fn shelf_collection_versions(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLShelfCollectionVersion>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        shelf_collection_versions_impl(state, user.user_id, id).await
    }

    /// What one of the caller's collections held at an earlier version.
    async fn shelf_collection_version_entries(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        version: i32,
    ) -> GraphQLResult<Vec<GraphQLVersionEntry>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        shelf_collection_version_entries_impl(state, user.user_id, id, version).await
    }
}

#[cfg(test)]
#[path = "mutations_shelf_collections_tests.rs"]
mod tests;
