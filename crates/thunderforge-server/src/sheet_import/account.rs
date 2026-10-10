//! What an account's end does to the sheets it brought (spec 048 T085).
//!
//! The files follow the account that uploaded them (the plan's open item 1):
//! they are deleted with it. The rows go inside the deletion's transaction,
//! and the objects after it commits, so a deletion that rolled back never
//! loses a file.
//!
//! A character rescued out of a deleted world (`collections::rescue`) keeps
//! the record of what was imported onto it. Where the file was the departing
//! account's, the record stays and its version link is cleared by the
//! foreign key, which the history shows as "file no longer kept".

use std::collections::HashMap;

use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_types;
use serde_json::Value;
use uuid::Uuid;

use super::ActorImportKind;
use super::snapshot::ActorState;
use crate::schema::{actor_imports, brought_characters};

/// What deleting an account's sheets left to do after commit.
#[derive(Debug, Default)]
pub struct Forgotten {
    /// Brought characters deleted, with their versions.
    pub characters: i64,
    /// The stored files to delete once the transaction commits.
    pub keys: Vec<String>,
}

/// Deletes `user_id`'s brought characters (their versions cascade) and hands
/// what they wrote in worlds that remain to each world's owner, the way
/// `shape_cleanup` hands back an edited shape. Run inside the deletion's
/// transaction, after the account's own worlds go.
pub(crate) fn forget_sheets_of_sync(
    conn: &mut PgConnection,
    user_id: Uuid,
) -> QueryResult<Forgotten> {
    let keys = super::storage::file_keys_of_sync(conn, user_id)?
        .into_iter()
        .map(|(_, _, key)| key)
        .collect();

    // Columns that reference the user with no ON DELETE action. A record
    // the account wrote in someone else's world stays, under that world's
    // owner, rather than holding the deletion back.
    for (table, column) in [
        ("actor_imports", "created_by"),
        ("actor_imports", "updated_by"),
        ("world_staged_content", "player_user_id"),
        ("world_staged_content", "created_by"),
        ("world_staged_content", "updated_by"),
        ("world_staged_content", "decided_by"),
    ] {
        diesel::sql_query(format!(
            "UPDATE {table} AS t SET {column} = w.created_by \
             FROM worlds AS w WHERE t.world_id = w.id AND t.{column} = $1"
        ))
        .bind::<sql_types::Uuid, _>(user_id)
        .execute(conn)?;
    }

    let characters = diesel::delete(
        brought_characters::table.filter(brought_characters::owner_user_id.eq(user_id)),
    )
    .execute(conn)? as i64;
    Ok(Forgotten { characters, keys })
}

/// Deletes the stored files of an account whose deletion has committed.
/// Best effort, as `feedback::schedule` purges: a file that will not delete
/// is logged by key prefix and left, never a reason the deletion failed.
pub async fn delete_sheet_files(state: &crate::AppState, keys: &[String]) {
    if keys.is_empty() {
        return;
    }
    let cfg = crate::storage::rustfs::RustFsConfig::resolve(state).await;
    for key in keys {
        if let Err(error) = crate::storage::rustfs::delete_object(&cfg, key).await {
            tracing::warn!(%error, "a deleted account's sheet file was not deleted");
        }
    }
}

type ImportRow = (
    Uuid,
    Uuid,
    Option<Uuid>,
    ActorImportKind,
    Option<Uuid>,
    Value,
    Value,
    Option<String>,
    NaiveDateTime,
);

/// Copies the import records of rescued actors onto their copies in
/// `destination`, oldest first so a rollback's `restored_from` is mapped
/// before it is needed. The version link is kept: a file the player brought
/// stays theirs, and one the departing account brought is cleared when that
/// account's versions are deleted.
///
/// The snapshot's links are pointed at the copied abilities and items; a
/// link to staged content is dropped, as the rescue drops the link itself.
pub(crate) fn keep_imports_of_rescued_sync(
    conn: &mut PgConnection,
    player: Uuid,
    destination: Uuid,
    actor_map: &HashMap<Uuid, Uuid>,
    ability_map: &HashMap<Uuid, Uuid>,
    item_map: &HashMap<Uuid, Uuid>,
) -> QueryResult<usize> {
    let sources: Vec<Uuid> = actor_map.keys().copied().collect();
    let rows: Vec<ImportRow> = actor_imports::table
        .filter(actor_imports::actor_id.eq_any(&sources))
        .order((actor_imports::applied_at.asc(), actor_imports::id.asc()))
        .select((
            actor_imports::id,
            actor_imports::actor_id,
            actor_imports::version_id,
            actor_imports::kind,
            actor_imports::restored_from,
            actor_imports::before_snapshot,
            actor_imports::written,
            actor_imports::plan_hash,
            actor_imports::applied_at,
        ))
        .load(conn)?;

    let mut ids: HashMap<Uuid, Uuid> = HashMap::new();
    for (id, actor, version, kind, restored_from, before, written, hash, applied_at) in rows {
        let Some(copy) = actor_map.get(&actor) else {
            continue;
        };
        let new_id = Uuid::new_v4();
        ids.insert(id, new_id);
        diesel::insert_into(actor_imports::table)
            .values((
                actor_imports::id.eq(new_id),
                actor_imports::world_id.eq(destination),
                actor_imports::actor_id.eq(*copy),
                actor_imports::version_id.eq(version),
                actor_imports::kind.eq(kind),
                actor_imports::restored_from.eq(restored_from.and_then(|r| ids.get(&r).copied())),
                actor_imports::before_snapshot.eq(remap_snapshot(before, ability_map, item_map)),
                actor_imports::written.eq(written),
                actor_imports::plan_hash.eq(hash),
                actor_imports::applied_at.eq(applied_at),
                actor_imports::created_by.eq(player),
                actor_imports::updated_by.eq(player),
            ))
            .execute(conn)?;
    }
    Ok(ids.len())
}

/// The snapshot with its links pointed at the copies. A snapshot that does
/// not parse is kept as it was: rolling back drops a link it cannot find.
fn remap_snapshot(
    before: Value,
    ability_map: &HashMap<Uuid, Uuid>,
    item_map: &HashMap<Uuid, Uuid>,
) -> Value {
    let Ok(mut state) = serde_json::from_value::<ActorState>(before.clone()) else {
        return before;
    };
    state.abilities.retain(|link| link.staged_id.is_none());
    for link in &mut state.abilities {
        link.ability_id = link.ability_id.and_then(|a| ability_map.get(&a).copied());
    }
    state.inventory.retain(|link| link.staged_id.is_none());
    for link in &mut state.inventory {
        link.item_id = link.item_id.and_then(|i| item_map.get(&i).copied());
    }
    serde_json::to_value(state).unwrap_or(before)
}
