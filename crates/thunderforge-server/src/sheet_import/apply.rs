//! Applying a reviewed sheet to an actor (contracts/graphql-sheet-import.md,
//! `applySheetImport`).
//!
//! The server reads the file itself, plans again and refuses unless the plan
//! is the one the person reviewed. The file is stored first; everything else
//! lands in one transaction, and the file is deleted if that fails.

use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use diesel::prelude::*;
use serde_json::{Map, Value, json};
use thunderforge_canvas_core::system_contribution::contribution_for;
use thunderforge_pdf::{Document, Limits, PdfError};
use thunderforge_sheet_import::{
    ContentChange, ContentTarget, ImportPlan, ImportedCharacter, PlanCertainty, Resolution,
    SheetMapping, normalise_name, plan_hash,
};
use uuid::Uuid;

use super::error::SheetImportError;
use super::index::staged_kind;
use super::preview::{parse_corrections, plan_for, require_may_import};
use super::snapshot::{self, ActorContext};
use super::{ActorImport, ActorImportKind, NewActorImport, NewSheetImportVersion};
use crate::compendium::origin::ContentOrigin;
use crate::graphql::permissioned_entity_resolvers::{PausableContent, refuse_content_if_paused};
use crate::staged_content::{NewStagedContent, StagedState};
use crate::state::AppState;
use crate::storage::rustfs::{RustFsConfig, delete_object, write_object};

/// The heading the unmapped values sit under in the notes. A re-import
/// replaces everything from it on.
pub const NOTES_HEADING: &str = "From the imported sheet";

/// What `applySheetImport` carries.
pub struct ApplyInput {
    pub actor_id: Uuid,
    pub bytes: Vec<u8>,
    pub corrections: Option<Value>,
    pub overwrite_play_state: Vec<String>,
    pub plan_hash: String,
}

/// The server's own reading and the plan made from it.
struct Planned {
    ctx: ActorContext,
    system: String,
    mapping: SheetMapping,
    plan: ImportPlan,
    reading: Value,
    pages: i16,
    character_id: Uuid,
    new_character: bool,
    version_no: i32,
    name: String,
}

pub async fn apply_sheet_import_impl(
    state: &AppState,
    systems_dir: &str,
    user_id: Uuid,
    is_admin: bool,
    input: ApplyInput,
) -> Result<ActorImport, SheetImportError> {
    require_may_import(state, user_id, is_admin, input.actor_id).await?;
    refuse_content_if_paused(state, PausableContent::Actor(input.actor_id))
        .await
        .map_err(SheetImportError::from_permission)?;
    let limits = Limits::default();
    if input.bytes.len() > limits.max_bytes {
        // Before the body is parsed, in the reader's own words.
        let error = thunderforge_sheet_import::ReadError::Pdf(PdfError::TooLarge {
            bytes: input.bytes.len(),
            limit: limits.max_bytes,
        });
        return Err(SheetImportError::sheet(error.code(), error.sentence()));
    }
    let corrections_value = input.corrections.clone().unwrap_or_else(|| json!({}));
    let corrections = parse_corrections(Some(&corrections_value))?;

    let pool = state.db_pool.clone();
    let planned = {
        let systems_dir = systems_dir.to_string();
        let bytes = input.bytes.clone();
        let pool = pool.clone();
        let expected = input.plan_hash.clone();
        let actor_id = input.actor_id;
        blocking(move || {
            let mut conn = pool.get().map_err(db)?;
            let ctx = snapshot::load(&mut conn, actor_id).map_err(db)?;
            let system = ctx
                .actor
                .game_system_id
                .clone()
                .ok_or(SheetImportError::NoMapping)?;
            let (reading, reading_value) = read_natively(&system, &bytes)?;
            let pages = Document::from_bytes_bounded(&bytes, Limits::default())
                .map(|doc| doc.page_count().min(i16::MAX as usize) as i16)
                .unwrap_or(1)
                .max(1);
            let (mapping, plan) = plan_for(&mut conn, &systems_dir, &ctx, &reading, &corrections)?;
            if plan_hash(&plan) != expected {
                return Err(SheetImportError::PlanChanged);
            }
            let (character_id, new_character, version_no) =
                character_for(&mut conn, actor_id, user_id)?;
            let name = reading
                .identity
                .name
                .value
                .clone()
                .unwrap_or_else(|| ctx.actor.label.clone());
            Ok(Planned {
                ctx,
                system,
                mapping,
                plan,
                reading: reading_value,
                pages,
                character_id,
                new_character,
                version_no,
                name,
            })
        })
        .await?
    };

    let key = super::storage::object_key(user_id, planned.character_id, planned.version_no);
    let cfg = RustFsConfig::resolve(state).await;
    let sha = thunderforge_sheet_import::hash::sha256_hex(&input.bytes);
    let file_bytes = input.bytes.len() as i32;
    write_object(&cfg, &key, input.bytes, "application/pdf")
        .await
        .map_err(|e| SheetImportError::Storage(e.to_string()))?;

    let overwrite: BTreeSet<String> = input.overwrite_play_state.into_iter().collect();
    let written = {
        let key = key.clone();
        blocking(move || {
            let mut conn = pool.get().map_err(db)?;
            let file = StoredFile {
                key: &key,
                sha256: &sha,
                bytes: file_bytes,
                corrections: &corrections_value,
            };
            write_import(&mut conn, user_id, &planned, &file, &overwrite)
        })
        .await
    };
    match written {
        Ok(record) => Ok(record),
        Err(error) => {
            let _ = delete_object(&cfg, &key).await;
            Err(error)
        }
    }
}

fn db(error: impl std::fmt::Display) -> SheetImportError {
    SheetImportError::Database(error.to_string())
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, SheetImportError> + Send + 'static,
) -> Result<T, SheetImportError> {
    tokio::task::spawn_blocking(work).await.map_err(db)?
}

/// The system's readers in order: one that does not recognise the document
/// passes it on; any other refusal is the answer.
pub fn read_natively(
    system: &str,
    bytes: &[u8],
) -> Result<(ImportedCharacter, Value), SheetImportError> {
    let slot = contribution_for(system)
        .and_then(|contribution| contribution.sheet_import)
        .ok_or(SheetImportError::NoMapping)?;
    let mut last = SheetImportError::sheet(
        "SHEET_NOT_RECOGNISED",
        "This is not a sheet this game system can read.",
    );
    for reader in slot.readers {
        match reader.read(bytes) {
            Ok(json) => {
                let value: Value = serde_json::from_str(&json)
                    .map_err(|e| SheetImportError::sheet("SHEET_UNREADABLE", e.to_string()))?;
                let reading = serde_json::from_value(value.clone())
                    .map_err(|e| SheetImportError::sheet("SHEET_UNREADABLE", e.to_string()))?;
                return Ok((reading, value));
            }
            Err(failure) if failure.code == "SHEET_NOT_RECOGNISED" => {
                last = SheetImportError::sheet(failure.code, failure.message);
            }
            Err(failure) => return Err(SheetImportError::sheet(failure.code, failure.message)),
        }
    }
    Err(last)
}

/// The character this importer brought onto this actor before, or a new
/// one, with the next version number.
fn character_for(
    conn: &mut PgConnection,
    actor_id: Uuid,
    owner: Uuid,
) -> Result<(Uuid, bool, i32), SheetImportError> {
    use crate::schema::{actor_imports, brought_characters, sheet_import_versions as versions};
    let existing = actor_imports::table
        .inner_join(versions::table.inner_join(brought_characters::table))
        .filter(actor_imports::actor_id.eq(actor_id))
        .filter(brought_characters::owner_user_id.eq(owner))
        .order(actor_imports::applied_at.desc())
        .select(brought_characters::id)
        .first::<Uuid>(conn)
        .optional()
        .map_err(db)?;
    let Some(character) = existing else {
        return Ok((Uuid::new_v4(), true, 1));
    };
    let last = versions::table
        .filter(versions::character_id.eq(character))
        .select(diesel::dsl::max(versions::version_no))
        .first::<Option<i32>>(conn)
        .map_err(db)?;
    Ok((character, false, last.unwrap_or(0) + 1))
}

struct StoredFile<'a> {
    key: &'a str,
    sha256: &'a str,
    bytes: i32,
    corrections: &'a Value,
}

/// Everything the import writes, in one transaction.
fn write_import(
    conn: &mut PgConnection,
    user_id: Uuid,
    planned: &Planned,
    file: &StoredFile<'_>,
    overwrite: &BTreeSet<String>,
) -> Result<ActorImport, SheetImportError> {
    use crate::schema::{actor_imports, brought_characters, sheet_import_versions, world_actors};

    let actor = &planned.ctx.actor;
    let before = serde_json::to_value(&planned.ctx.state).map_err(db)?;
    conn.transaction::<_, SheetImportError, _>(|conn| {
        if planned.new_character {
            diesel::insert_into(brought_characters::table)
                .values((
                    brought_characters::id.eq(planned.character_id),
                    brought_characters::owner_user_id.eq(user_id),
                    brought_characters::system_id.eq(&planned.system),
                    brought_characters::name.eq(&planned.name),
                    brought_characters::created_by.eq(user_id),
                    brought_characters::updated_by.eq(user_id),
                ))
                .execute(conn)
                .map_err(db)?;
        }
        let version_id = diesel::insert_into(sheet_import_versions::table)
            .values(NewSheetImportVersion {
                character_id: planned.character_id,
                version_no: planned.version_no,
                file_key: file.key,
                file_sha256: file.sha256,
                file_bytes: file.bytes,
                file_pages: planned.pages,
                reader_id: &planned.plan.reader_id,
                reader_version: &planned.plan.reader_version,
                reading: &planned.reading,
                corrections: file.corrections,
                created_by: user_id,
                updated_by: user_id,
            })
            .returning(sheet_import_versions::id)
            .get_result::<Uuid>(conn)
            .map_err(db)?;

        let fields = write_fields(conn, user_id, planned, overwrite)?;
        let (links, staged) = write_links(conn, user_id, planned)?;
        let corrected: Vec<&str> = planned
            .plan
            .fields
            .iter()
            .filter(|f| f.certainty == PlanCertainty::Corrected)
            .map(|f| f.path.as_str())
            .collect();
        let written = json!({
            "fields": fields,
            "links": links,
            "staged": staged,
            "corrected": corrected,
        });

        diesel::update(world_actors::table.find(actor.id))
            .set((
                world_actors::origin.eq(ContentOrigin::Uploaded),
                world_actors::updated_at.eq(Utc::now().naive_utc()),
            ))
            .execute(conn)
            .map_err(db)?;

        let hash = plan_hash(&planned.plan);
        let record = diesel::insert_into(actor_imports::table)
            .values(NewActorImport {
                world_id: actor.world_id,
                actor_id: actor.id,
                version_id: Some(version_id),
                kind: ActorImportKind::Import,
                restored_from: None,
                before_snapshot: &before,
                written: &written,
                plan_hash: Some(&hash),
                created_by: user_id,
                updated_by: user_id,
            })
            .returning(ActorImport::as_returning())
            .get_result(conn)
            .map_err(db)?;

        crate::world_events::record_world_event(
            conn,
            actor.world_id,
            crate::world_events::EVENT_CODE_SHEET_IMPORT_APPLIED,
            Some(json!({ "actorId": actor.id, "importId": record.id })),
            user_id,
        )
        .map_err(|e| SheetImportError::Database(e.message))?;
        Ok(record)
    })
}

/// A value as the notes show it.
fn note_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// The notes with the unmapped values under [`NOTES_HEADING`], replacing
/// an earlier block.
pub fn notes_with_unmapped(current: Option<&str>, lines: &[(String, String)]) -> String {
    let kept = current
        .map(|text| match text.find(NOTES_HEADING) {
            Some(at) => text[..at].trim_end().to_string(),
            None => text.trim_end().to_string(),
        })
        .unwrap_or_default();
    if lines.is_empty() {
        return kept;
    }
    let mut out = kept;
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(NOTES_HEADING);
    for (label, value) in lines {
        out.push_str(&format!("\n- {label}: {value}"));
    }
    out
}

/// The sheet's fields onto the actor's label and system data, each data
/// type through its validator. Returns the targets written.
fn write_fields(
    conn: &mut PgConnection,
    user_id: Uuid,
    planned: &Planned,
    overwrite: &BTreeSet<String>,
) -> Result<Vec<String>, SheetImportError> {
    use crate::schema::{world_actor_system_data as data, world_actors};

    let mut objects: BTreeMap<String, Map<String, Value>> = planned.ctx.data_objects();
    let mut touched = BTreeSet::new();
    let mut targets = Vec::new();
    for field in planned.plan.writes(overwrite) {
        if field.target == "actor.label" {
            if let Some(Value::String(label)) = &field.new {
                diesel::update(world_actors::table.find(planned.ctx.actor.id))
                    .set(world_actors::label.eq(label))
                    .execute(conn)
                    .map_err(db)?;
                targets.push(field.target.clone());
            }
            continue;
        }
        let Some((data_type, key)) = field.target.split_once('.') else {
            continue;
        };
        let Some(object) = objects.get_mut(data_type) else {
            continue;
        };
        object.insert(key.to_string(), field.new.clone().unwrap_or(Value::Null));
        touched.insert(data_type.to_string());
        targets.push(field.target.clone());
    }

    if let Some((data_type, key)) = planned.mapping.notes.split_once('.')
        && let Some(object) = objects.get_mut(data_type)
    {
        let lines: Vec<(String, String)> = planned
            .plan
            .unmapped
            .iter()
            .map(|row| (row.label.clone(), note_text(&row.value)))
            .collect();
        let current = object.get(key).and_then(Value::as_str);
        let next = notes_with_unmapped(current, &lines);
        if current.unwrap_or_default() != next {
            object.insert(key.to_string(), Value::String(next));
            touched.insert(data_type.to_string());
            targets.push(planned.mapping.notes.clone());
        }
    }

    let now = Utc::now().naive_utc();
    for data_type in &touched {
        let value = Value::Object(objects.remove(data_type).unwrap_or_default());
        crate::systems::validate_actor_system_data(&planned.system, data_type, &value)
            .map_err(|e| SheetImportError::Invalid(format!("{data_type}: {e}")))?;
        let column = |v: &Value| -> [Option<Value>; 5] {
            let mut out: [Option<Value>; 5] = Default::default();
            if let Some(at) = snapshot::DATA_TYPES.iter().position(|t| t == data_type) {
                out[at] = Some(v.clone());
            }
            out
        };
        let [a, r, p, t, s] = column(&value);
        let exists = data::table
            .filter(data::actor_id.eq(planned.ctx.actor.id))
            .select(data::id)
            .first::<Uuid>(conn)
            .optional()
            .map_err(db)?;
        if let Some(id) = exists {
            let target = data::table.find(id);
            let set = (data::updated_by.eq(user_id), data::updated_at.eq(now));
            match data_type.as_str() {
                "ability_data" => diesel::update(target)
                    .set((data::ability_data.eq(a), set))
                    .execute(conn),
                "resource_data" => diesel::update(target)
                    .set((data::resource_data.eq(r), set))
                    .execute(conn),
                "proficiency_data" => diesel::update(target)
                    .set((data::proficiency_data.eq(p), set))
                    .execute(conn),
                "trait_data" => diesel::update(target)
                    .set((data::trait_data.eq(t), set))
                    .execute(conn),
                _ => diesel::update(target)
                    .set((data::spell_data.eq(s), set))
                    .execute(conn),
            }
            .map_err(db)?;
        } else {
            diesel::insert_into(data::table)
                .values((
                    data::actor_id.eq(planned.ctx.actor.id),
                    data::game_system_id.eq(&planned.system),
                    data::ability_data.eq(a),
                    data::resource_data.eq(r),
                    data::proficiency_data.eq(p),
                    data::trait_data.eq(t),
                    data::spell_data.eq(s),
                    data::created_by.eq(user_id),
                    data::updated_by.eq(user_id),
                ))
                .execute(conn)
                .map_err(db)?;
        }
    }
    Ok(targets)
}

fn truncate(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// Where a change's link points: world content, or a staged piece.
enum LinkTarget {
    World(Uuid),
    Staged(Uuid),
}

/// The staged piece for a change, upserted by its unique key. A piece that
/// was adopted since points at what it became.
fn staged_piece(
    conn: &mut PgConnection,
    user_id: Uuid,
    planned: &Planned,
    change: &ContentChange,
    differs_from: Option<Uuid>,
) -> Result<LinkTarget, SheetImportError> {
    use crate::schema::world_staged_content as staged;
    let kind = staged_kind(change);
    let name = truncate(&change.name, 200);
    let normalised = truncate(&change.normalised, 200);
    let (id, state, ability, item) = diesel::insert_into(staged::table)
        .values(NewStagedContent {
            world_id: planned.ctx.actor.world_id,
            player_user_id: user_id,
            kind: &truncate(&kind, 32),
            name: &name,
            normalized_name: &normalised,
            content_hash: &change.content_hash,
            field_values: &change.fields,
            origin: ContentOrigin::Uploaded,
            differs_from,
            first_actor_id: Some(planned.ctx.actor.id),
            created_by: user_id,
            updated_by: user_id,
        })
        .on_conflict((
            staged::world_id,
            staged::kind,
            staged::normalized_name,
            staged::content_hash,
        ))
        .do_update()
        .set(staged::updated_by.eq(user_id))
        .returning((
            staged::id,
            staged::state,
            staged::adopted_ability_id,
            staged::adopted_item_id,
        ))
        .get_result::<(Uuid, StagedState, Option<Uuid>, Option<Uuid>)>(conn)
        .map_err(db)?;
    Ok(match (state, ability.or(item)) {
        (StagedState::Adopted, Some(adopted)) => LinkTarget::World(adopted),
        _ => LinkTarget::Staged(id),
    })
}

fn parse_id(id: &str) -> Result<Uuid, SheetImportError> {
    Uuid::parse_str(id).map_err(|e| SheetImportError::Invalid(e.to_string()))
}

/// The sheet's content as links: world content by id, everything else
/// staged (FR-031, FR-033). Returns the link ids written and the staged
/// pieces they point at.
fn write_links(
    conn: &mut PgConnection,
    user_id: Uuid,
    planned: &Planned,
) -> Result<(Vec<Uuid>, Vec<Uuid>), SheetImportError> {
    let mut links = Vec::new();
    let mut staged = Vec::new();
    for change in &planned.plan.content {
        let item = match change.target {
            ContentTarget::Item => true,
            ContentTarget::Ability { .. } => false,
            ContentTarget::Refine => continue,
        };
        if change.removed {
            remove_link(conn, planned, change, item)?;
            continue;
        }
        let target = match &change.resolution {
            Resolution::World { id } => LinkTarget::World(parse_id(id)?),
            Resolution::StagedExisting { id } => LinkTarget::Staged(parse_id(id)?),
            Resolution::StagedNew => staged_piece(conn, user_id, planned, change, None)?,
            Resolution::Differs { id } => {
                staged_piece(conn, user_id, planned, change, Some(parse_id(id)?))?
            }
        };
        if let LinkTarget::Staged(id) = &target
            && !staged.contains(id)
        {
            staged.push(*id);
        }
        let link = if item {
            write_item_link(conn, user_id, planned, change, &target)?
        } else {
            write_ability_link(conn, planned, change, &target)?
        };
        links.push(link);
    }
    Ok((links, staged))
}

fn remove_link(
    conn: &mut PgConnection,
    planned: &Planned,
    change: &ContentChange,
    item: bool,
) -> Result<(), SheetImportError> {
    use crate::schema::{world_actor_abilities as abilities, world_actor_inventory as inventory};
    let id = match &change.resolution {
        Resolution::World { id } | Resolution::StagedExisting { id } => parse_id(id)?,
        _ => return Ok(()),
    };
    let actor = planned.ctx.actor.id;
    if item {
        diesel::delete(
            inventory::table
                .filter(inventory::actor_id.eq(actor))
                .filter(inventory::item_id.eq(id).or(inventory::staged_id.eq(id))),
        )
        .execute(conn)
    } else {
        diesel::delete(
            abilities::table
                .filter(abilities::actor_id.eq(actor))
                .filter(abilities::ability_id.eq(id).or(abilities::staged_id.eq(id))),
        )
        .execute(conn)
    }
    .map_err(db)?;
    Ok(())
}

fn write_ability_link(
    conn: &mut PgConnection,
    planned: &Planned,
    change: &ContentChange,
    target: &LinkTarget,
) -> Result<Uuid, SheetImportError> {
    use crate::schema::world_actor_abilities as abilities;
    let (ability_id, staged_id) = match target {
        LinkTarget::World(id) => (Some(*id), None),
        LinkTarget::Staged(id) => (None, Some(*id)),
    };
    let granted_by = (!change.link.granted_by.is_empty())
        .then(|| truncate(&change.link.granted_by.join(", "), 200));
    let recharge = change.link.recharge.as_deref().map(|r| truncate(r, 16));
    let uses_max = change.link.uses.map(|u| u.clamp(0, i16::MAX as i32) as i16);
    let existing = planned
        .ctx
        .state
        .abilities
        .iter()
        .find(|link| normalise_name(&link.name) == change.normalised)
        .map(|link| link.id);
    let now = Utc::now().naive_utc();
    match existing {
        Some(id) => diesel::update(abilities::table.find(id))
            .set((
                abilities::ability_id.eq(ability_id),
                abilities::staged_id.eq(staged_id),
                abilities::ability_name_snapshot.eq(&change.name),
                abilities::prepared.eq(change.link.prepared),
                abilities::granted_by.eq(&granted_by),
                abilities::uses_max.eq(uses_max),
                abilities::recharge.eq(&recharge),
                abilities::updated_at.eq(now),
            ))
            .returning(abilities::id)
            .get_result(conn),
        None => diesel::insert_into(abilities::table)
            .values((
                abilities::actor_id.eq(planned.ctx.actor.id),
                abilities::ability_id.eq(ability_id),
                abilities::staged_id.eq(staged_id),
                abilities::ability_name_snapshot.eq(&change.name),
                abilities::prepared.eq(change.link.prepared),
                abilities::granted_by.eq(&granted_by),
                abilities::uses_max.eq(uses_max),
                abilities::recharge.eq(&recharge),
            ))
            .returning(abilities::id)
            .get_result(conn),
    }
    .map_err(db)
}

fn write_item_link(
    conn: &mut PgConnection,
    user_id: Uuid,
    planned: &Planned,
    change: &ContentChange,
    target: &LinkTarget,
) -> Result<Uuid, SheetImportError> {
    use crate::schema::world_actor_inventory as inventory;
    let (item_id, staged_id) = match target {
        LinkTarget::World(id) => (Some(*id), None),
        LinkTarget::Staged(id) => (None, Some(*id)),
    };
    let quantity = change.link.quantity.unwrap_or(1).max(0);
    let equipped = change.link.equipped.unwrap_or(false);
    let attuned = change.link.attuned.unwrap_or(false);
    let existing = planned
        .ctx
        .state
        .inventory
        .iter()
        .find(|link| normalise_name(&link.name) == change.normalised)
        .map(|link| link.id);
    let now = Utc::now().naive_utc();
    match existing {
        Some(id) => diesel::update(inventory::table.find(id))
            .set((
                inventory::item_id.eq(item_id),
                inventory::staged_id.eq(staged_id),
                inventory::item_name_snapshot.eq(&change.name),
                inventory::quantity.eq(quantity),
                inventory::equipped.eq(equipped),
                inventory::attuned.eq(attuned),
                inventory::updated_by.eq(Some(user_id)),
                inventory::updated_at.eq(now),
            ))
            .returning(inventory::id)
            .get_result(conn),
        None => diesel::insert_into(inventory::table)
            .values((
                inventory::actor_id.eq(planned.ctx.actor.id),
                inventory::item_id.eq(item_id),
                inventory::staged_id.eq(staged_id),
                inventory::item_name_snapshot.eq(&change.name),
                inventory::quantity.eq(quantity),
                inventory::equipped.eq(equipped),
                inventory::attuned.eq(attuned),
                inventory::created_by.eq(Some(user_id)),
                inventory::updated_by.eq(Some(user_id)),
            ))
            .returning(inventory::id)
            .get_result(conn),
    }
    .map_err(db)
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod tests;
