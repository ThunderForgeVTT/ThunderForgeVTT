//! Rolling an actor back to before an import (contracts/graphql-sheet-import.md,
//! `rollBackActor`).
//!
//! The target import's before-snapshot is the actor as it was. The sheet's
//! fields and links return to it; the play-state values the system declares
//! keep what the table has now (FR-044a). Only the world's GM may do this
//! (FR-044b), and the rollback is itself a record, so it can be undone too.

use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use diesel::prelude::*;
use serde_json::{Value, json};
use uuid::Uuid;

use super::error::SheetImportError;
use super::snapshot::{self, AbilityLink, ActorState, DATA_TYPES, InventoryLink};
use super::{ActorImport, ActorImportKind, NewActorImport};
use crate::auth::world_membership::actor_in_world;
use crate::play_pause::gate::refuse_if_paused;
use crate::staged_content::StagedState;
use crate::state::AppState;

/// The play-state entry that keeps an ability's spent uses.
const LINK_USES: &str = "links.uses_used";

fn db(error: impl std::fmt::Display) -> SheetImportError {
    SheetImportError::Database(error.to_string())
}

pub async fn roll_back_actor_impl(
    state: &AppState,
    systems_dir: &str,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
    to_import_id: Uuid,
) -> Result<ActorImport, SheetImportError> {
    let pool = state.db_pool.clone();
    let systems_dir = systems_dir.to_string();
    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(db)?;
        roll_back(
            &mut conn,
            &systems_dir,
            user_id,
            is_admin,
            actor_id,
            to_import_id,
        )
    })
    .await
    .map_err(db)?
}

fn roll_back(
    conn: &mut PgConnection,
    systems_dir: &str,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
    to_import_id: Uuid,
) -> Result<ActorImport, SheetImportError> {
    use crate::schema::{actor_imports, world_actors};

    let world_id = world_actors::table
        .find(actor_id)
        .select(world_actors::world_id)
        .first::<Uuid>(conn)
        .optional()
        .map_err(db)?
        .ok_or_else(|| SheetImportError::Forbidden("You are not at this table.".into()))?;
    if !actor_in_world(conn, user_id, is_admin, world_id).runs_the_world() {
        return Err(SheetImportError::Forbidden(
            "Only the Game Master can roll a character back.".into(),
        ));
    }
    refuse_if_paused(conn, world_id)?;
    let target = actor_imports::table
        .find(to_import_id)
        .select(ActorImport::as_select())
        .first(conn)
        .optional()
        .map_err(db)?
        .filter(|import| import.actor_id == actor_id)
        .ok_or_else(|| SheetImportError::Invalid("that import is not this character's".into()))?;
    let restored: ActorState = serde_json::from_value(target.before_snapshot.clone())
        .map_err(|e| SheetImportError::Invalid(format!("the snapshot does not read: {e}")))?;

    let ctx = snapshot::load(conn, actor_id).map_err(db)?;
    let play_state = super::preview::mapping_for(systems_dir, &ctx.actor)
        .map(|(_, mapping)| mapping.play_state)
        .unwrap_or_default();
    let system = ctx.actor.game_system_id.clone().unwrap_or_default();
    let before = serde_json::to_value(&ctx.state).map_err(db)?;

    conn.transaction::<_, SheetImportError, _>(|conn| {
        diesel::update(world_actors::table.find(actor_id))
            .set((
                world_actors::label.eq(&restored.label),
                world_actors::updated_at.eq(Utc::now().naive_utc()),
            ))
            .execute(conn)
            .map_err(db)?;
        let data = restore_data(&ctx.state, &restored, &play_state);
        write_data(conn, user_id, actor_id, &system, &data)?;
        let keep_uses = play_state.contains(LINK_USES);
        let abilities = restore_abilities(conn, actor_id, &ctx.state, &restored, keep_uses)?;
        let items = restore_inventory(conn, user_id, actor_id, &ctx.state, &restored)?;
        let written = json!({
            "dataTypes": data.keys().collect::<Vec<_>>(),
            "links": abilities.into_iter().chain(items).collect::<Vec<_>>(),
        });

        let record = diesel::insert_into(actor_imports::table)
            .values(NewActorImport {
                world_id,
                actor_id,
                version_id: None,
                kind: ActorImportKind::Rollback,
                restored_from: Some(to_import_id),
                before_snapshot: &before,
                written: &written,
                plan_hash: None,
                created_by: user_id,
                updated_by: user_id,
            })
            .returning(ActorImport::as_returning())
            .get_result(conn)
            .map_err(db)?;
        crate::world_events::record_world_event(
            conn,
            world_id,
            crate::world_events::EVENT_CODE_ACTOR_ROLLED_BACK,
            Some(json!({ "actorId": actor_id, "importId": record.id })),
            user_id,
        )
        .map_err(|e| SheetImportError::Database(e.message))?;
        Ok(record)
    })
}

/// Each data type as the snapshot holds it, with the play-state values
/// carried over from now. A type the snapshot has no data for is cleared:
/// there is no sheet to keep play state in.
pub fn restore_data(
    current: &ActorState,
    restored: &ActorState,
    play_state: &BTreeSet<String>,
) -> BTreeMap<String, Option<Value>> {
    DATA_TYPES
        .iter()
        .map(|data_type| {
            let value = restored.system_data.get(*data_type).map(|then| {
                let mut then = then.clone();
                if let Some(now) = current.system_data.get(*data_type) {
                    for target in play_state {
                        if let Some(path) = target
                            .strip_prefix(*data_type)
                            .and_then(|rest| rest.strip_prefix('.'))
                        {
                            let segments: Vec<&str> = path.split('.').collect();
                            keep_play_state(&mut then, now, &segments);
                        }
                    }
                }
                then
            });
            (data_type.to_string(), value)
        })
        .collect()
}

/// Copy the value at `path` from `now` into `then`; absent now means absent
/// after. A segment ending `[]` walks the array element by element.
pub fn keep_play_state(then: &mut Value, now: &Value, path: &[&str]) {
    let Some((first, rest)) = path.split_first() else {
        return;
    };
    let Some(then) = then.as_object_mut() else {
        return;
    };
    let now = now.as_object();
    if let Some(key) = first.strip_suffix("[]") {
        let (Some(Value::Array(then)), Some(Value::Array(now))) =
            (then.get_mut(key), now.and_then(|n| n.get(key)))
        else {
            return;
        };
        for (then, now) in then.iter_mut().zip(now) {
            keep_play_state(then, now, rest);
        }
        return;
    }
    let now = now.and_then(|n| n.get(*first));
    if rest.is_empty() {
        match now {
            Some(value) => {
                then.insert(first.to_string(), value.clone());
            }
            None => {
                then.remove(*first);
            }
        }
    } else if let (Some(then), Some(now)) = (then.get_mut(*first), now) {
        keep_play_state(then, now, rest);
    }
}

fn write_data(
    conn: &mut PgConnection,
    user_id: Uuid,
    actor_id: Uuid,
    system: &str,
    data: &BTreeMap<String, Option<Value>>,
) -> Result<(), SheetImportError> {
    use crate::schema::world_actor_system_data as columns;
    for (data_type, value) in data {
        if let Some(value) = value {
            crate::systems::validate_actor_system_data(system, data_type, value)
                .map_err(|e| SheetImportError::Invalid(format!("{data_type}: {e}")))?;
        }
    }
    let get = |t: &str| data.get(t).cloned().flatten();
    let (a, r, p, t, s) = (
        get("ability_data"),
        get("resource_data"),
        get("proficiency_data"),
        get("trait_data"),
        get("spell_data"),
    );
    let updated = diesel::update(columns::table.filter(columns::actor_id.eq(actor_id)))
        .set((
            columns::ability_data.eq(&a),
            columns::resource_data.eq(&r),
            columns::proficiency_data.eq(&p),
            columns::trait_data.eq(&t),
            columns::spell_data.eq(&s),
            columns::updated_by.eq(user_id),
            columns::updated_at.eq(Utc::now().naive_utc()),
        ))
        .execute(conn)
        .map_err(db)?;
    if updated == 0 && data.values().any(Option::is_some) {
        diesel::insert_into(columns::table)
            .values((
                columns::actor_id.eq(actor_id),
                columns::game_system_id.eq(system),
                columns::ability_data.eq(a),
                columns::resource_data.eq(r),
                columns::proficiency_data.eq(p),
                columns::trait_data.eq(t),
                columns::spell_data.eq(s),
                columns::created_by.eq(user_id),
                columns::updated_by.eq(user_id),
            ))
            .execute(conn)
            .map_err(db)?;
    }
    Ok(())
}

/// Where a snapshot's link points now: staged content adopted since points
/// at what it became. `None` when what it pointed at is gone.
fn resolve(
    conn: &mut PgConnection,
    world: Option<Uuid>,
    staged_id: Option<Uuid>,
    item: bool,
) -> Result<Option<(Option<Uuid>, Option<Uuid>)>, SheetImportError> {
    use crate::schema::{world_abilities, world_items, world_staged_content as staged};
    if let Some(id) = staged_id {
        let row = staged::table
            .find(id)
            .select((
                staged::state,
                staged::adopted_ability_id,
                staged::adopted_item_id,
            ))
            .first::<(StagedState, Option<Uuid>, Option<Uuid>)>(conn)
            .optional()
            .map_err(db)?;
        return Ok(match row {
            None => None,
            Some((StagedState::Adopted, ability, adopted_item)) => {
                match if item { adopted_item } else { ability } {
                    Some(adopted) => Some((Some(adopted), None)),
                    None => Some((None, Some(id))),
                }
            }
            Some(_) => Some((None, Some(id))),
        });
    }
    let Some(id) = world else {
        return Ok(Some((None, None)));
    };
    let exists = if item {
        diesel::select(diesel::dsl::exists(world_items::table.find(id))).get_result::<bool>(conn)
    } else {
        diesel::select(diesel::dsl::exists(world_abilities::table.find(id)))
            .get_result::<bool>(conn)
    }
    .map_err(db)?;
    Ok(exists.then_some((Some(id), None)))
}

fn restore_abilities(
    conn: &mut PgConnection,
    actor_id: Uuid,
    current: &ActorState,
    restored: &ActorState,
    keep_uses: bool,
) -> Result<Vec<Uuid>, SheetImportError> {
    use crate::schema::world_actor_abilities as links;
    let keep: Vec<Uuid> = restored.abilities.iter().map(|l| l.id).collect();
    diesel::delete(
        links::table
            .filter(links::actor_id.eq(actor_id))
            .filter(links::id.ne_all(&keep)),
    )
    .execute(conn)
    .map_err(db)?;
    let now: BTreeMap<Uuid, &AbilityLink> = current.abilities.iter().map(|l| (l.id, l)).collect();
    let mut written = Vec::new();
    for link in &restored.abilities {
        let Some((ability_id, staged_id)) = resolve(conn, link.ability_id, link.staged_id, false)?
        else {
            continue;
        };
        let uses_used = match now.get(&link.id) {
            Some(current) if keep_uses => current.uses_used,
            _ => link.uses_used,
        };
        let values = (
            links::ability_id.eq(ability_id),
            links::staged_id.eq(staged_id),
            links::ability_name_snapshot.eq(&link.name),
            links::prepared.eq(link.prepared),
            links::granted_by.eq(&link.granted_by),
            links::uses_max.eq(link.uses_max),
            links::uses_used.eq(uses_used),
            links::recharge.eq(&link.recharge),
        );
        if now.contains_key(&link.id) {
            diesel::update(links::table.find(link.id))
                .set((values, links::updated_at.eq(Utc::now().naive_utc())))
                .execute(conn)
        } else {
            diesel::insert_into(links::table)
                .values((links::id.eq(link.id), links::actor_id.eq(actor_id), values))
                .execute(conn)
        }
        .map_err(db)?;
        written.push(link.id);
    }
    Ok(written)
}

fn restore_inventory(
    conn: &mut PgConnection,
    user_id: Uuid,
    actor_id: Uuid,
    current: &ActorState,
    restored: &ActorState,
) -> Result<Vec<Uuid>, SheetImportError> {
    use crate::schema::world_actor_inventory as inventory;
    let keep: Vec<Uuid> = restored.inventory.iter().map(|l| l.id).collect();
    diesel::delete(
        inventory::table
            .filter(inventory::actor_id.eq(actor_id))
            .filter(inventory::id.ne_all(&keep)),
    )
    .execute(conn)
    .map_err(db)?;
    let now: BTreeSet<Uuid> = current.inventory.iter().map(|l| l.id).collect();
    let mut written = Vec::new();
    for link in &restored.inventory {
        let InventoryLink {
            id,
            item_id,
            staged_id,
            name,
            quantity,
            equipped,
            attuned,
        } = link;
        let Some((item_id, staged_id)) = resolve(conn, *item_id, *staged_id, true)? else {
            continue;
        };
        let values = (
            inventory::item_id.eq(item_id),
            inventory::staged_id.eq(staged_id),
            inventory::item_name_snapshot.eq(name),
            inventory::quantity.eq(quantity),
            inventory::equipped.eq(equipped),
            inventory::attuned.eq(attuned),
            inventory::updated_by.eq(Some(user_id)),
        );
        if now.contains(id) {
            diesel::update(inventory::table.find(*id))
                .set((values, inventory::updated_at.eq(Utc::now().naive_utc())))
                .execute(conn)
        } else {
            diesel::insert_into(inventory::table)
                .values((
                    inventory::id.eq(*id),
                    inventory::actor_id.eq(actor_id),
                    inventory::created_by.eq(Some(user_id)),
                    values,
                ))
                .execute(conn)
        }
        .map_err(db)?;
        written.push(*id);
    }
    Ok(written)
}

#[cfg(test)]
#[path = "rollback_tests.rs"]
mod tests;
