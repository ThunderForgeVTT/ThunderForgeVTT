//! Deciding on what players brought (spec 048 FR-033 to FR-036c): adopt,
//! adopt all, decline and revisit, for the GM and a Trusted Player only.
//!
//! An adoption writes the world's own row, origin `Uploaded` and made by the
//! player who brought it, and repoints every link to the piece at it, so no
//! character holds a second copy (FR-034). Every decision records event 41,
//! the withdrawing event the report's staleness check compares against.

use chrono::Utc;
use diesel::prelude::*;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{StagedContent, StagedState};
use crate::auth::world_membership::{ManagesContentError, require_manages_content};
use crate::compendium::origin::ContentOrigin;
use crate::sheet_import::error::SheetImportError;
use crate::sheet_import::records::{Person, person};
use crate::state::AppState;
use crate::world_events::{EVENT_CODE_STAGED_CONTENT_DECIDED, record_world_event};

/// A piece as the queue shows it.
#[derive(Debug, Clone)]
pub struct StagedView {
    pub piece: StagedContent,
    pub brought_by: Person,
    /// Every actor linking it, by id and name.
    pub actors: Vec<(Uuid, String)>,
    pub decided_by: Option<Person>,
}

fn db(error: impl std::fmt::Display) -> SheetImportError {
    SheetImportError::Database(error.to_string())
}

fn refused(error: ManagesContentError) -> SheetImportError {
    match error {
        ManagesContentError::Database(message) => SheetImportError::Database(message),
        other => SheetImportError::Forbidden(other.to_string()),
    }
}

fn not_found() -> SheetImportError {
    SheetImportError::Forbidden("There is no such piece at this table.".into())
}

/// The piece, locked for the decision, after the caller's rule is checked.
fn load_for_decision(
    conn: &mut PgConnection,
    user_id: Uuid,
    staged_id: Uuid,
) -> Result<StagedContent, SheetImportError> {
    use crate::schema::world_staged_content as staged;
    let piece = staged::table
        .find(staged_id)
        .select(StagedContent::as_select())
        .for_update()
        .first(conn)
        .optional()
        .map_err(db)?
        .ok_or_else(not_found)?;
    require_manages_content(conn, piece.world_id, user_id).map_err(|e| match e {
        ManagesContentError::NotAMember => not_found(),
        other => refused(other),
    })?;
    Ok(piece)
}

/// The actors linking a piece, by id.
fn linked_actors(conn: &mut PgConnection, staged_id: Uuid) -> QueryResult<Vec<Uuid>> {
    use crate::schema::{world_actor_abilities as abilities, world_actor_inventory as inventory};
    let mut ids: Vec<Uuid> = abilities::table
        .filter(abilities::staged_id.eq(staged_id))
        .select(abilities::actor_id)
        .load(conn)?;
    ids.extend(
        inventory::table
            .filter(inventory::staged_id.eq(staged_id))
            .select(inventory::actor_id)
            .load::<Uuid>(conn)?,
    );
    ids.sort();
    ids.dedup();
    Ok(ids)
}

fn record_decision(
    conn: &mut PgConnection,
    user_id: Uuid,
    piece: &StagedContent,
    state: &str,
    actor_ids: &[Uuid],
) -> Result<(), SheetImportError> {
    record_world_event(
        conn,
        piece.world_id,
        EVENT_CODE_STAGED_CONTENT_DECIDED,
        Some(json!({ "stagedId": piece.id, "state": state, "actorIds": actor_ids })),
        user_id,
    )
    .map(|_| ())
    .map_err(|e| SheetImportError::Database(e.message))
}

fn text(fields: &Value, key: &str) -> Option<String> {
    fields
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn number(fields: &Value, key: &str) -> Option<f64> {
    let value = fields.get(key)?;
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.trim().parse().ok()))
}

/// The world's own row for a piece: an item, or an ability of the piece's
/// vocabulary type. Returns (ability id, item id).
fn write_world_row(
    conn: &mut PgConnection,
    user_id: Uuid,
    piece: &StagedContent,
) -> Result<(Option<Uuid>, Option<Uuid>), SheetImportError> {
    use crate::schema::{world_abilities, world_items};
    let fields = &piece.field_values;
    let description = text(fields, "description");
    if piece.kind == "item" {
        let id = diesel::insert_into(world_items::table)
            .values(crate::models::NewWorldItem {
                world_id: piece.world_id,
                name: piece.name.clone(),
                description,
                icon_asset_id: None,
                created_by: piece.player_user_id,
                origin: ContentOrigin::Uploaded,
                weight: number(fields, "weight"),
            })
            .returning(world_items::id)
            .get_result::<Uuid>(conn)
            .map_err(db)?;
        return Ok((None, Some(id)));
    }
    let grade = number(fields, "level")
        .filter(|level| (0.0..=99.0).contains(level))
        .map(|level| level as i32);
    let id = diesel::insert_into(world_abilities::table)
        .values(crate::models::NewWorldAbility {
            world_id: piece.world_id,
            name: piece.name.clone(),
            description,
            classification: piece.kind.chars().take(16).collect(),
            grade,
            gm_only: false,
            created_by: piece.player_user_id,
            updated_by: user_id,
            origin: ContentOrigin::Uploaded,
        })
        .returning(world_abilities::id)
        .get_result::<Uuid>(conn)
        .map_err(db)?;
    Ok((Some(id), None))
}

/// Adopt one locked, undecided-or-declined piece: the world row, the links
/// repointed, the state, the event.
fn adopt_locked(
    conn: &mut PgConnection,
    user_id: Uuid,
    piece: StagedContent,
) -> Result<StagedContent, SheetImportError> {
    use crate::schema::{
        world_actor_abilities as abilities, world_actor_inventory as inventory,
        world_staged_content as staged,
    };
    if piece.state == StagedState::Adopted {
        return Ok(piece);
    }
    let actor_ids = linked_actors(conn, piece.id).map_err(db)?;
    let (ability_id, item_id) = write_world_row(conn, user_id, &piece)?;
    super::attack::write(conn, &piece.field_values, ability_id, item_id).map_err(db)?;
    let now = Utc::now().naive_utc();
    if let Some(id) = ability_id {
        diesel::update(abilities::table.filter(abilities::staged_id.eq(piece.id)))
            .set((
                abilities::ability_id.eq(Some(id)),
                abilities::staged_id.eq(None::<Uuid>),
                abilities::updated_at.eq(now),
            ))
            .execute(conn)
            .map_err(db)?;
    }
    if let Some(id) = item_id {
        diesel::update(inventory::table.filter(inventory::staged_id.eq(piece.id)))
            .set((
                inventory::item_id.eq(Some(id)),
                inventory::staged_id.eq(None::<Uuid>),
                inventory::updated_by.eq(Some(user_id)),
                inventory::updated_at.eq(now),
            ))
            .execute(conn)
            .map_err(db)?;
    }
    let adopted = diesel::update(staged::table.find(piece.id))
        .set((
            staged::state.eq(StagedState::Adopted),
            staged::adopted_ability_id.eq(ability_id),
            staged::adopted_item_id.eq(item_id),
            staged::decided_by.eq(Some(user_id)),
            staged::decided_at.eq(Some(now)),
            staged::updated_by.eq(user_id),
        ))
        .returning(StagedContent::as_returning())
        .get_result(conn)
        .map_err(db)?;
    record_decision(conn, user_id, &adopted, "adopted", &actor_ids)?;
    Ok(adopted)
}

/// Move a piece to pending or declined, with its event.
fn set_state(
    conn: &mut PgConnection,
    user_id: Uuid,
    piece: &StagedContent,
    to: StagedState,
) -> Result<StagedContent, SheetImportError> {
    use crate::schema::world_staged_content as staged;
    let actor_ids = linked_actors(conn, piece.id).map_err(db)?;
    let now = Utc::now().naive_utc();
    let row = diesel::update(staged::table.find(piece.id))
        .set((
            staged::state.eq(to),
            staged::decided_by.eq(Some(user_id)),
            staged::decided_at.eq(Some(now)),
            staged::updated_by.eq(user_id),
        ))
        .returning(StagedContent::as_returning())
        .get_result(conn)
        .map_err(db)?;
    let label = match to {
        StagedState::Declined => "declined",
        _ => "pending",
    };
    record_decision(conn, user_id, &row, label, &actor_ids)?;
    Ok(row)
}

fn already_adopted() -> SheetImportError {
    SheetImportError::Invalid("it is in the world's books already".into())
}

/// `adoptStagedContent`.
pub fn adopt(
    conn: &mut PgConnection,
    user_id: Uuid,
    staged_id: Uuid,
) -> Result<StagedContent, SheetImportError> {
    conn.transaction(|conn| {
        let piece = load_for_decision(conn, user_id, staged_id)?;
        adopt_locked(conn, user_id, piece)
    })
}

/// `adoptAllStagedContent`: the player's pending pieces as they stand now,
/// in one transaction. Anything staged afterwards stays pending (FR-033a).
pub fn adopt_all(
    conn: &mut PgConnection,
    user_id: Uuid,
    world_id: Uuid,
    player_id: Uuid,
) -> Result<Vec<StagedContent>, SheetImportError> {
    use crate::schema::world_staged_content as staged;
    conn.transaction(|conn| {
        require_manages_content(conn, world_id, user_id).map_err(refused)?;
        let pieces = staged::table
            .filter(staged::world_id.eq(world_id))
            .filter(staged::player_user_id.eq(player_id))
            .filter(staged::state.eq(StagedState::Pending))
            .order((staged::created_at.asc(), staged::id.asc()))
            .select(StagedContent::as_select())
            .for_update()
            .load(conn)
            .map_err(db)?;
        pieces
            .into_iter()
            .map(|piece| adopt_locked(conn, user_id, piece))
            .collect()
    })
}

/// `declineStagedContent`. An adopted piece is the world's now, and is
/// managed in the compendium instead.
pub fn decline(
    conn: &mut PgConnection,
    user_id: Uuid,
    staged_id: Uuid,
) -> Result<StagedContent, SheetImportError> {
    conn.transaction(|conn| {
        let piece = load_for_decision(conn, user_id, staged_id)?;
        match piece.state {
            StagedState::Adopted => Err(already_adopted()),
            StagedState::Declined => Ok(piece),
            StagedState::Pending => set_state(conn, user_id, &piece, StagedState::Declined),
        }
    })
}

/// `revisitStagedContent`: a declined piece goes back to pending, or
/// straight to adopted (FR-036b).
pub fn revisit(
    conn: &mut PgConnection,
    user_id: Uuid,
    staged_id: Uuid,
    to: StagedState,
) -> Result<StagedContent, SheetImportError> {
    conn.transaction(|conn| {
        let piece = load_for_decision(conn, user_id, staged_id)?;
        if piece.state != StagedState::Declined {
            return Err(SheetImportError::Invalid(
                "only a declined piece can be revisited".into(),
            ));
        }
        match to {
            StagedState::Adopted => adopt_locked(conn, user_id, piece),
            StagedState::Pending => set_state(conn, user_id, &piece, StagedState::Pending),
            StagedState::Declined => Ok(piece),
        }
    })
}

/// `stagedContent`: everything for the GM and a Trusted Player; a Player
/// sees only what they brought.
pub fn list(
    conn: &mut PgConnection,
    viewer: Uuid,
    world_id: Uuid,
    state: Option<StagedState>,
    player: Option<Uuid>,
) -> Result<Vec<StagedContent>, SheetImportError> {
    use crate::schema::world_staged_content as staged;
    let only_own = match require_manages_content(conn, world_id, viewer) {
        Ok(_) => false,
        Err(ManagesContentError::NotPermitted) => true,
        Err(other) => return Err(refused(other)),
    };
    let mut query = staged::table
        .filter(staged::world_id.eq(world_id))
        .select(StagedContent::as_select())
        .order((staged::created_at.asc(), staged::id.asc()))
        .into_boxed();
    if only_own {
        query = query.filter(staged::player_user_id.eq(viewer));
    }
    if let Some(player) = player {
        query = query.filter(staged::player_user_id.eq(player));
    }
    if let Some(state) = state {
        query = query.filter(staged::state.eq(state));
    }
    query.load(conn).map_err(db)
}

/// A piece with the people and actors the queue names.
pub fn view(conn: &mut PgConnection, piece: StagedContent) -> Result<StagedView, SheetImportError> {
    use crate::schema::world_actors;
    let actor_ids = linked_actors(conn, piece.id).map_err(db)?;
    let mut actors: Vec<(Uuid, String)> = world_actors::table
        .filter(world_actors::id.eq_any(&actor_ids))
        .select((world_actors::id, world_actors::label))
        .load(conn)
        .map_err(db)?;
    actors.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    Ok(StagedView {
        brought_by: person(conn, piece.player_user_id).map_err(db)?,
        decided_by: piece
            .decided_by
            .map(|id| person(conn, id))
            .transpose()
            .map_err(db)?,
        actors,
        piece,
    })
}

/// Run a decision or a read on a pooled connection, off the async runtime,
/// and return the pieces as the queue shows them.
pub async fn blocking<F>(state: &AppState, work: F) -> Result<Vec<StagedView>, SheetImportError>
where
    F: FnOnce(&mut PgConnection) -> Result<Vec<StagedContent>, SheetImportError> + Send + 'static,
{
    let pool = state.db_pool.clone();
    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(db)?;
        let pieces = work(&mut conn)?;
        pieces
            .into_iter()
            .map(|piece| view(&mut conn, piece))
            .collect()
    })
    .await
    .map_err(db)?
}
