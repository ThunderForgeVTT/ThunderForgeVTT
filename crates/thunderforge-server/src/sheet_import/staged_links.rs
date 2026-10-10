//! The links an actor holds to staged pieces, for the sheet to mark
//! "awaiting the GM" (spec 048 FR-034). The play reads withhold these links;
//! this read is only for showing them.

use diesel::prelude::*;
use uuid::Uuid;

use super::error::SheetImportError;
use super::records::require_sees_actor;
use crate::staged_content::StagedState;
use crate::state::AppState;

/// One link to a piece the world does not hold yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedLink {
    /// The link row: an actor ability or an inventory entry.
    pub id: Uuid,
    pub staged_id: Uuid,
    /// A vocabulary type, or `item`.
    pub kind: String,
    pub name: String,
    pub state: StagedState,
}

/// Every staged link on the actor, by name.
pub fn staged_links_for(conn: &mut PgConnection, actor_id: Uuid) -> QueryResult<Vec<StagedLink>> {
    use crate::schema::{
        world_actor_abilities as abilities, world_actor_inventory as inventory,
        world_staged_content as staged,
    };
    let columns = (staged::kind, staged::name, staged::state);
    let mut links: Vec<StagedLink> = abilities::table
        .inner_join(staged::table)
        .filter(abilities::actor_id.eq(actor_id))
        .select((abilities::id, staged::id, columns))
        .load::<(Uuid, Uuid, (String, String, StagedState))>(conn)?
        .into_iter()
        .chain(
            inventory::table
                .inner_join(staged::table)
                .filter(inventory::actor_id.eq(actor_id))
                .select((inventory::id, staged::id, columns))
                .load::<(Uuid, Uuid, (String, String, StagedState))>(conn)?,
        )
        .map(|(id, staged_id, (kind, name, state))| StagedLink {
            id,
            staged_id,
            kind,
            name,
            state,
        })
        .collect();
    links.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    Ok(links)
}

/// `actorStagedLinks`: anyone who may see the actor, as the sheet is.
pub async fn actor_staged_links_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> Result<Vec<StagedLink>, SheetImportError> {
    require_sees_actor(state, user_id, is_admin, actor_id).await?;
    let pool = state.db_pool.clone();
    tokio::task::spawn_blocking(move || {
        let mut conn = pool
            .get()
            .map_err(|e| SheetImportError::Database(e.to_string()))?;
        Ok(staged_links_for(&mut conn, actor_id)?)
    })
    .await
    .map_err(|e| SheetImportError::Database(e.to_string()))?
}
