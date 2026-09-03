//! `abilityVocabulary(worldId)` — what this world calls its abilities.
//!
//! Spec 033 FR-003 to FR-006, FR-011a. One assembled answer, so that every
//! surface naming an ability type uses the same word for it. Six web
//! components used to read the manifest and cast `abilityFacets` themselves;
//! FR-006 requires all of them to agree, and six readers is six chances not
//! to.
//!
//! Readable by any world member. The vocabulary is not secret — FR-010 says
//! the tab set and its labels are identical for GMs and players, and only the
//! abilities within them differ.

use async_graphql::{Context, Object, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::ability_vocabulary::{AbilityVocabulary, for_system};
use crate::auth::world_membership::require_world_member;
use crate::graphql::{app_state, authenticated_user};
use crate::schema::{world_abilities, worlds};

#[derive(Default)]
pub struct AbilityVocabularyQuery;

#[Object]
impl AbilityVocabularyQuery {
    /// The umbrella term and the ability types this world presents.
    ///
    /// Assembled per world rather than per system, because FR-011a makes
    /// *presence* depend on what the world holds: a built-in the active system
    /// never mentions still gets a tab if the world has one of them, so no
    /// ability is ever hidden by the rule that stops a 5e world carrying empty
    /// "Powers" and "Talents".
    async fn ability_vocabulary(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<AbilityVocabulary> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;

        // The world's system, and the classifications it actually holds. Both
        // in one blocking hop: the second is what FR-011a turns on, and asking
        // for it separately would be two round trips to answer one question.
        let (system_id, in_use) = tokio::task::spawn_blocking(move || {
            // Membership first, on the same connection: the vocabulary is not
            // secret between a world's members (FR-010), but it is not for
            // people who are not in the world.
            require_world_member(&mut conn, user_id, world_id)
                .map_err(|_| diesel::result::Error::NotFound)?;

            let system_id: Option<String> = worlds::table
                .filter(worlds::id.eq(world_id))
                .select(worlds::game_system_id)
                .first::<Option<String>>(&mut conn)?;

            let in_use: Vec<String> = world_abilities::table
                .filter(world_abilities::world_id.eq(world_id))
                .select(world_abilities::classification)
                .distinct()
                .load::<String>(&mut conn)?;

            Ok::<_, diesel::result::Error>((system_id, in_use))
        })
        .await
        .map_err(|_| async_graphql::Error::new("Failed to spawn blocking task"))?
        .map_err(|_| async_graphql::Error::new("Failed to read the world's abilities"))?;

        Ok(for_system(
            &state.directories.systems_dir,
            system_id.as_deref(),
            &in_use,
        ))
    }
}
