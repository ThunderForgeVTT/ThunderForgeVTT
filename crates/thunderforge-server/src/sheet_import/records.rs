//! An actor's imports and rollbacks as the history shows them
//! (contracts/graphql-sheet-import.md, `actorImports`).

use diesel::prelude::*;
use uuid::Uuid;

use super::ActorImport;
use super::error::SheetImportError;
use crate::auth::actor_permissions::require_actor_permission;
use crate::auth::world_membership::actor_in_world;
use crate::graphql::types::ActorPermissionLevel;
use crate::state::AppState;

/// Who applied an import, as the history names them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
}

#[derive(Debug, Clone)]
pub struct ImportRecord {
    pub import: ActorImport,
    pub applied_by: Person,
    pub version_no: Option<i32>,
    /// Paths the importer corrected (FR-022).
    pub corrected: Vec<String>,
    /// The file may be downloaded by this viewer: the owner or the GM.
    pub file_available: bool,
}

fn person(conn: &mut PgConnection, id: Uuid) -> QueryResult<Person> {
    use crate::schema::users;
    let (username, first, last) = users::table
        .find(id)
        .select((users::username, users::first_name, users::last_name))
        .first::<(String, Option<String>, Option<String>)>(conn)?;
    let full = [first, last]
        .into_iter()
        .flatten()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    Ok(Person {
        id,
        display_name: if full.is_empty() {
            username.clone()
        } else {
            full
        },
        username,
    })
}

/// Every record on the actor, newest first, as `viewer` may see them.
pub fn records_for(
    conn: &mut PgConnection,
    viewer: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> Result<Vec<ImportRecord>, SheetImportError> {
    use crate::schema::{actor_imports, brought_characters, sheet_import_versions as versions};
    let rows = actor_imports::table
        .left_join(versions::table.left_join(brought_characters::table))
        .filter(actor_imports::actor_id.eq(actor_id))
        .order((actor_imports::applied_at.desc(), actor_imports::id.desc()))
        .select((
            ActorImport::as_select(),
            versions::version_no.nullable(),
            brought_characters::owner_user_id.nullable(),
        ))
        .load::<(ActorImport, Option<i32>, Option<Uuid>)>(conn)?;
    let Some(world_id) = rows.first().map(|(import, _, _)| import.world_id) else {
        return Ok(Vec::new());
    };
    let runs_the_world = actor_in_world(conn, viewer, is_admin, world_id).runs_the_world();
    rows.into_iter()
        .map(|(import, version_no, owner)| {
            let corrected = import
                .written
                .get("corrected")
                .and_then(|v| v.as_array())
                .map(|paths| {
                    paths
                        .iter()
                        .filter_map(|p| p.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let file_available = version_no.is_some() && (runs_the_world || owner == Some(viewer));
            Ok(ImportRecord {
                applied_by: person(conn, import.created_by)?,
                version_no,
                corrected,
                file_available,
                import,
            })
        })
        .collect()
}

/// One record, as `viewer` sees it.
pub fn record(
    conn: &mut PgConnection,
    viewer: Uuid,
    is_admin: bool,
    import: &ActorImport,
) -> Result<ImportRecord, SheetImportError> {
    records_for(conn, viewer, is_admin, import.actor_id)?
        .into_iter()
        .find(|record| record.import.id == import.id)
        .ok_or_else(|| SheetImportError::Database("the import record vanished".into()))
}

/// Viewer or above on the actor, the actor visible to them, and a seat at
/// its table. The permission ladder's floor is Viewer for anyone, so the
/// seat is checked here: a stranger reads nothing about a character.
pub async fn require_sees_actor(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> Result<(), SheetImportError> {
    require_actor_permission(
        state,
        user_id,
        is_admin,
        actor_id,
        ActorPermissionLevel::Viewer,
    )
    .await
    .map_err(SheetImportError::from_permission)?;
    crate::auth::npc_visibility::require_actor_visible(state, user_id, is_admin, actor_id)
        .await
        .map_err(SheetImportError::from_permission)?;
    let pool = state.db_pool.clone();
    let seated = tokio::task::spawn_blocking(move || {
        use crate::schema::world_actors;
        let mut conn = pool
            .get()
            .map_err(|e| SheetImportError::Database(e.to_string()))?;
        let world_id = world_actors::table
            .find(actor_id)
            .select(world_actors::world_id)
            .first::<Uuid>(&mut conn)?;
        Ok::<_, SheetImportError>(
            actor_in_world(&mut conn, user_id, is_admin, world_id)
                .role
                .is_some()
                || is_admin,
        )
    })
    .await
    .map_err(|e| SheetImportError::Database(e.to_string()))??;
    if seated {
        Ok(())
    } else {
        Err(SheetImportError::Forbidden(
            "You are not at this table.".into(),
        ))
    }
}

/// `actorImports`: anyone who may see the actor ([`require_sees_actor`]).
/// Not behind the flag, like downloads: switching imports off does not hide
/// what was imported.
pub async fn actor_imports_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> Result<Vec<ImportRecord>, SheetImportError> {
    require_sees_actor(state, user_id, is_admin, actor_id).await?;
    let pool = state.db_pool.clone();
    tokio::task::spawn_blocking(move || {
        let mut conn = pool
            .get()
            .map_err(|e| SheetImportError::Database(e.to_string()))?;
        records_for(&mut conn, user_id, is_admin, actor_id)
    })
    .await
    .map_err(|e| SheetImportError::Database(e.to_string()))?
}
