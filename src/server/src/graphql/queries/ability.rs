//! Spec 025: Ability catalog queries (`worldAbilities`, `ability`,
//! `suggestAbilityName`). See contracts/graphql-abilities.md.
//!
//! **Every query here filters `gm_only` for non-DM callers.** That is a
//! security boundary, not a UI convenience — a miss on any one of them is a
//! content leak (FR-024b, SC-004a). data-model.md carries the full surface
//! table, including the paths owned by other modules.

use async_graphql::Context;
use diesel::dsl::sql;
use diesel::sql_types::{Bool, Double, Text};

use crate::auth::actor_permissions::is_dm_of_world;
use crate::graphql::types::{GraphQLAbility, GraphQLAbilityEffect};
use crate::graphql::*;
use crate::models::WorldAbility;
use crate::schema::world_abilities;
use crate::state::AppState;

/// Delegates to the single loader in `mutations_abilities`, rather than
/// re-declaring a private copy the way `queries/item.rs` and
/// `mutations_items.rs` each do.
async fn load_ability_effects(
    state: &AppState,
    ability_id: uuid::Uuid,
) -> GraphQLResult<Vec<GraphQLAbilityEffect>> {
    Ok(
        crate::graphql::mutations_abilities::load_ability_effects(state, ability_id)
            .await?
            .into_iter()
            .map(GraphQLAbilityEffect::from)
            .collect(),
    )
}

async fn to_graphql_ability(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    row: WorldAbility,
) -> GraphQLResult<GraphQLAbility> {
    let effects = load_ability_effects(state, row.id).await?;
    let my_permission_level = crate::auth::ability_permissions::effective_ability_permission(
        state, user_id, is_admin, row.id,
    )
    .await?;
    Ok(GraphQLAbility::from_row(row, effects, my_permission_level))
}

/// Testable core of `AbilityQuery::world_abilities`.
///
/// Every world member sees every *visible* ability at at least Viewer level
/// (FR-005). GM-only abilities are excluded entirely for non-DMs (FR-024b);
/// `search` filters `name`/`description` case-insensitively, matching the item
/// catalog.
pub async fn world_abilities_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    world_id: uuid::Uuid,
    search: Option<String>,
) -> GraphQLResult<Vec<WorldAbility>> {
    require_visible_world(state, user_id, is_admin, world_id).await?;

    let caller_is_dm = is_dm_of_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let rows = tokio::task::spawn_blocking(move || {
        let mut query = world_abilities::table
            .filter(world_abilities::world_id.eq(world_id))
            .into_boxed();

        // FR-024b: hidden abilities never reach a non-DM's list or search.
        if !caller_is_dm {
            query = query.filter(world_abilities::gm_only.eq(false));
        }

        if let Some(term) = search.as_ref().filter(|s| !s.trim().is_empty()) {
            let pattern = format!("%{}%", term.replace('%', "\\%").replace('_', "\\_"));
            query = query.filter(
                world_abilities::name
                    .ilike(pattern.clone())
                    .or(world_abilities::description.ilike(pattern)),
            );
        }

        query
            .order(world_abilities::name.asc())
            .select(WorldAbility::as_select())
            .load::<WorldAbility>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load world abilities"))?;

    // Spec 015: excluded from list queries entirely when disabled.
    crate::moderation::filter_visible(state, "world_ability", rows, |a| a.id).await
}

/// Testable core of `AbilityQuery::ability` (FR-025).
///
/// A GM-only ability is rejected for a non-DM with the **same** error a
/// nonexistent id produces. That symmetry is deliberate: a distinguishable
/// error would let a non-DM probe ids to discover which hidden abilities exist.
pub async fn ability_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    ability_id: uuid::Uuid,
) -> GraphQLResult<WorldAbility> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let row = tokio::task::spawn_blocking(move || {
        world_abilities::table
            .filter(world_abilities::id.eq(ability_id))
            .select(WorldAbility::as_select())
            .first::<WorldAbility>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load ability"))?;

    let Some(row) = row else {
        return Err(Error::new("Ability not found"));
    };

    require_visible_world(state, user_id, is_admin, row.world_id).await?;

    if row.gm_only && !is_dm_of_world(state, user_id, is_admin, row.world_id).await? {
        // Same message as the missing-row branch above — do not differentiate.
        return Err(Error::new("Ability not found"));
    }

    Ok(row)
}

/// Testable core of `AbilityQuery::suggest_ability_name` (FR-007).
///
/// Advisory only — never gates `createAbility`. Uses `pg_trgm`'s `similarity()`
/// against the trigram GIN index, reusing the extension spec 013 already
/// enabled. Filters `gm_only` for non-DMs so the suggestion list cannot be used
/// to discover hidden ability names.
pub async fn suggest_ability_name_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    world_id: uuid::Uuid,
    name: String,
) -> GraphQLResult<Vec<WorldAbility>> {
    require_visible_world(state, user_id, is_admin, world_id).await?;

    let caller_is_dm = is_dm_of_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        let mut query = world_abilities::table
            .filter(world_abilities::world_id.eq(world_id))
            .into_boxed();

        if !caller_is_dm {
            query = query.filter(world_abilities::gm_only.eq(false));
        }

        query
            .filter(
                sql::<Bool>("similarity(name, ")
                    .bind::<Text, _>(name.clone())
                    .sql(") > 0.4"),
            )
            .order(
                sql::<Double>("similarity(name, ")
                    .bind::<Text, _>(name)
                    .sql(")")
                    .desc(),
            )
            .limit(5)
            .select(WorldAbility::as_select())
            .load::<WorldAbility>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to search ability names"))
}

#[derive(Default)]
pub struct AbilityQuery;

#[async_graphql::Object]
impl AbilityQuery {
    async fn world_abilities(
        &self,
        ctx: &Context<'_>,
        world_id: uuid::Uuid,
        search: Option<String>,
    ) -> GraphQLResult<Vec<GraphQLAbility>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let rows = world_abilities_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            search,
        )
        .await?;
        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            result
                .push(to_graphql_ability(state, auth_user.user_id, auth_user.is_admin, row).await?);
        }
        Ok(result)
    }

    async fn ability(
        &self,
        ctx: &Context<'_>,
        ability_id: uuid::Uuid,
    ) -> GraphQLResult<GraphQLAbility> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let row = ability_impl(state, auth_user.user_id, auth_user.is_admin, ability_id).await?;

        // Spec 015: single-entity queries return a moderation placeholder
        // rather than excluding the row, for every caller including the owner.
        if crate::moderation::effective_status(state, "world_ability", row.id)
            .await?
            .is_some()
        {
            let my_permission_level =
                crate::auth::ability_permissions::effective_ability_permission(
                    state,
                    auth_user.user_id,
                    auth_user.is_admin,
                    row.id,
                )
                .await?;
            let case_id = crate::moderation::active_case_id(state, "world_ability", row.id).await?;
            return Ok(GraphQLAbility::moderated_placeholder(
                row.id,
                row.world_id,
                my_permission_level,
                case_id,
            ));
        }

        to_graphql_ability(state, auth_user.user_id, auth_user.is_admin, row).await
    }

    async fn suggest_ability_name(
        &self,
        ctx: &Context<'_>,
        world_id: uuid::Uuid,
        name: String,
    ) -> GraphQLResult<Vec<GraphQLAbility>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let rows = suggest_ability_name_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            name,
        )
        .await?;
        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            result
                .push(to_graphql_ability(state, auth_user.user_id, auth_user.is_admin, row).await?);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use diesel::prelude::*;

    fn make_ability(
        conn: &mut diesel::PgConnection,
        world_id: uuid::Uuid,
        creator: uuid::Uuid,
        name: &str,
        gm_only: bool,
    ) -> uuid::Uuid {
        diesel::insert_into(world_abilities::table)
            .values((
                world_abilities::world_id.eq(world_id),
                world_abilities::name.eq(name),
                world_abilities::classification.eq("spell"),
                world_abilities::gm_only.eq(gm_only),
                world_abilities::created_by.eq(creator),
                world_abilities::updated_by.eq(creator),
            ))
            .returning(world_abilities::id)
            .get_result::<uuid::Uuid>(conn)
            .expect("insert ability")
    }

    /// FR-005: a plain world member can browse the world's abilities.
    #[tokio::test]
    async fn world_abilities_returns_all_abilities_for_a_member() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let member_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, member_id, "Player");
        make_ability(&mut conn, world_id, owner_id, "Fireball", false);
        make_ability(&mut conn, world_id, owner_id, "Cleave", false);
        drop(conn);

        let rows = world_abilities_impl(&state, member_id, false, world_id, None)
            .await
            .expect("member may list abilities");
        assert_eq!(rows.len(), 2);
        // FR-005: ordered by name.
        assert_eq!(rows[0].name, "Cleave");
        assert_eq!(rows[1].name, "Fireball");
    }

    /// FR-007: advisory only, and it must find a near miss.
    #[tokio::test]
    async fn suggest_ability_name_finds_close_matches() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        make_ability(&mut conn, world_id, owner_id, "Fireball", false);
        drop(conn);

        let hits = suggest_ability_name_impl(
            &state,
            owner_id,
            false,
            world_id,
            "Firebal".to_string(),
        )
        .await
        .expect("suggest should run");
        assert_eq!(hits.len(), 1, "a near-miss name should be suggested");
        assert_eq!(hits[0].name, "Fireball");

        let none = suggest_ability_name_impl(
            &state,
            owner_id,
            false,
            world_id,
            "Completely Unrelated".to_string(),
        )
        .await
        .unwrap();
        assert!(none.is_empty(), "an unrelated name should suggest nothing");
    }

    /// FR-024b/SC-004a: the leak sweep. A GM-only ability must be absent from
    /// every non-DM-reachable query in this module, and `ability`'s rejection
    /// must be indistinguishable from a nonexistent id.
    #[tokio::test]
    async fn gm_only_ability_is_absent_from_every_non_dm_surface() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let member_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, member_id, "Player");
        let secret_id = make_ability(&mut conn, world_id, owner_id, "Soul Harvest", true);
        make_ability(&mut conn, world_id, owner_id, "Cleave", false);
        drop(conn);

        // 1. List excludes it.
        let listed = world_abilities_impl(&state, member_id, false, world_id, None)
            .await
            .unwrap();
        assert_eq!(listed.len(), 1, "player must not see the GM-only ability");
        assert_eq!(listed[0].name, "Cleave");

        // 2. Search by its exact name finds nothing.
        let searched = world_abilities_impl(
            &state,
            member_id,
            false,
            world_id,
            Some("Soul Harvest".to_string()),
        )
        .await
        .unwrap();
        assert!(searched.is_empty(), "search must not surface a hidden ability");

        // 3. Name suggestions exclude it.
        let suggested = suggest_ability_name_impl(
            &state,
            member_id,
            false,
            world_id,
            "Soul Harvest".to_string(),
        )
        .await
        .unwrap();
        assert!(
            suggested.is_empty(),
            "suggestions must not leak hidden ability names"
        );

        // 4. Detail is denied, and indistinguishably from a missing id.
        let hidden_err = ability_impl(&state, member_id, false, secret_id)
            .await
            .expect_err("player must be denied the hidden ability");
        let missing_err = ability_impl(&state, member_id, false, uuid::Uuid::now_v7())
            .await
            .expect_err("a nonexistent id must also error");
        assert_eq!(
            hidden_err.message, missing_err.message,
            "hidden and nonexistent must be indistinguishable, or ids can be probed"
        );

        // 5. The DM still sees everything.
        let dm_listed = world_abilities_impl(&state, owner_id, false, world_id, None)
            .await
            .unwrap();
        assert_eq!(dm_listed.len(), 2, "the DM sees their own hidden ability");
        ability_impl(&state, owner_id, false, secret_id)
            .await
            .expect("the DM may open their own hidden ability");
    }

    /// SC-004/FR-025: enforcement is server-side, not UI gating.
    ///
    /// Note what this can and cannot assert. The ownership block's lowest level
    /// is `Viewer`, which is also the default — so there is no "below Viewer"
    /// state and a world member can never be denied an *ordinary* ability.
    /// Denial of detail data therefore comes from two places only: GM-only
    /// visibility, and world membership. Both are checked here directly against
    /// the resolver, with no UI involved.
    #[tokio::test]
    async fn ability_detail_is_denied_without_viewer_access() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let member_id = insert_test_user(&mut conn);
        let outsider_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, member_id, "Player");
        let open_id = make_ability(&mut conn, world_id, owner_id, "Open Book", false);
        let secret_id = make_ability(&mut conn, world_id, owner_id, "Sealed", true);
        drop(conn);

        // A member reaches an ordinary ability — the ownership block cannot
        // deny this, by construction.
        ability_impl(&state, member_id, false, open_id)
            .await
            .expect("a world member may open a visible ability");

        // ...but not a GM-only one.
        ability_impl(&state, member_id, false, secret_id)
            .await
            .expect_err("a member must be denied a GM-only ability");

        // A non-member is denied even the ordinary one.
        ability_impl(&state, outsider_id, false, open_id)
            .await
            .expect_err("a non-member must be denied entirely");

        // The DM reaches both.
        ability_impl(&state, owner_id, false, open_id).await.unwrap();
        ability_impl(&state, owner_id, false, secret_id).await.unwrap();
    }
}
