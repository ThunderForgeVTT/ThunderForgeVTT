//! Creating, changing, hiding and launching a scene.

use async_graphql::{Context, Error, Result as GraphQLResult};

use super::*;
use crate::state::AppState;

/// Testable core of `SceneMutation::update_scene_hidden` (spec 022,
/// FR-007/FR-019). Unlike `update_scene` (owner-of-scene-gated,
/// pre-existing), this uses the broader `is_dm_of_world` check — any
/// GM/Owner of the scene's world may toggle visibility, not just whoever
/// created the scene, matching spec.md's "GM/Owner members" wording.
pub async fn update_scene_hidden_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    scene_id: uuid::Uuid,
    hidden: bool,
) -> GraphQLResult<GraphQLScene> {
    use crate::schema::scenes;
    use diesel::prelude::*;

    let mut lookup_conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = tokio::task::spawn_blocking(move || {
        scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .select(scenes::world_id)
            .first::<uuid::Uuid>(&mut lookup_conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Scene not found"))?;

    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new(
            "Only the DM (Owner or GM) may change a scene's visibility",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let updated_scene = tokio::task::spawn_blocking(move || {
        diesel::update(scenes::table.filter(scenes::scene_id.eq(scene_id)))
            .set(scenes::hidden.eq(hidden))
            .returning(crate::models::Scene::as_returning())
            .get_result(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to update scene visibility"))?;

    Ok(GraphQLScene::from(updated_scene))
}

/// Testable core of `SceneMutation::launch_scene` (spec 022,
/// FR-002a/FR-002b/FR-002c, ADR-046). Sets the world's server-authoritative
/// active scene and broadcasts the change over the existing `world_events`
/// transport so every world member currently in Play live-switches to it
/// (research.md §6).
pub async fn launch_scene_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    world_id: uuid::Uuid,
    scene_id: uuid::Uuid,
) -> GraphQLResult<GraphQLWorld> {
    use crate::schema::{scenes, worlds};
    use diesel::prelude::*;

    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("Only the DM (Owner or GM) may launch a scene"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let updated_world = tokio::task::spawn_blocking(move || {
        // The scene must belong to this world — a GM of world A must
        // not be able to point world B's active scene at one of A's
        // scenes (FR-002b implicitly assumes same-world).
        let scene_world_id = scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .select(scenes::world_id)
            .first::<uuid::Uuid>(&mut conn)?;

        if scene_world_id != world_id {
            return Err(diesel::result::Error::RollbackTransaction);
        }

        let world = diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::active_scene_id.eq(scene_id))
            .returning(World::as_returning())
            .get_result::<World>(&mut conn)?;

        crate::world_events::record_world_event(
            &mut conn,
            world_id,
            crate::world_events::EVENT_CODE_SCENE_LAUNCHED,
            Some(serde_json::json!({ "sceneId": scene_id.to_string() })),
            user_id,
        )
        .map_err(|_| diesel::result::Error::RollbackTransaction)?;

        Ok(world)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to launch scene — it may not belong to this world"))?;

    Ok(GraphQLWorld::from(updated_world))
}

#[derive(Default)]
pub struct SceneMutation;

#[async_graphql::Object]
impl SceneMutation {
    async fn create_scene(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateSceneInput,
    ) -> GraphQLResult<GraphQLScene> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;

        // Creating a scene had **no membership check at all**: any signed-in
        // user could add a scene to any world by naming its id. Scene
        // authoring is a Game Master power, so it takes the same gate every
        // other content mutation takes.
        if !crate::auth::world_membership::is_dm_of_world(
            state,
            user_id,
            auth_user.is_admin,
            input.world_id,
        )
        .await?
        {
            return Err(Error::new("Forbidden"));
        }

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let scene_id = uuid::Uuid::now_v7();
        let world_id = input.world_id;
        let explicit_grid_type = input.grid_type;
        let name = input.name;
        let description = input.description;
        let type_ = input.type_.unwrap_or_else(|| "battlemap".to_string());
        let grid_size = input.grid_size.unwrap_or(5);
        let width = input.width.unwrap_or(100);
        let height = input.height.unwrap_or(100);
        let metadata = input.metadata.map(|j| j.0);

        let inserted_scene = tokio::task::spawn_blocking(move || {
            use crate::schema::{scenes, worlds};
            use diesel::prelude::*;

            // Spec 022 (FR-015): a scene that doesn't explicitly choose a
            // grid type inherits the world's configured default instead of
            // always defaulting to "square" — the world row is already the
            // source of truth for this, no separate lookup table needed.
            let grid_type = match explicit_grid_type {
                Some(gt) => gt,
                None => worlds::table
                    .filter(worlds::id.eq(world_id))
                    .select(worlds::default_scene_grid_type)
                    .first::<String>(&mut conn)
                    .unwrap_or_else(|_| "square".to_string()),
            };

            let new_scene = crate::models::Scene {
                scene_id,
                world_id,
                name,
                description,
                type_,
                grid_size,
                grid_type,
                width,
                height,
                metadata,
                owner_id: user_id,
                created_at: now,
                updated_at: now,
                background_image_path: None,
                background_asset_id: None,
                summary_markdown: None,
                summary_rendered_html: None,
                // Spec 022 (FR-003, Clarifications): every newly created
                // scene starts hidden regardless of caller input — there is
                // no `hidden` field on this input type by design, so a
                // GM must explicitly un-hide via `updateSceneHidden`.
                hidden: true,
                preview_asset_id: None,
            };

            let values = (
                scenes::scene_id.eq(new_scene.scene_id),
                scenes::world_id.eq(new_scene.world_id),
                scenes::name.eq(&new_scene.name),
                scenes::description.eq(&new_scene.description),
                scenes::type_.eq(&new_scene.type_),
                scenes::grid_size.eq(new_scene.grid_size),
                scenes::grid_type.eq(&new_scene.grid_type),
                scenes::width.eq(new_scene.width),
                scenes::height.eq(new_scene.height),
                scenes::metadata.eq(&new_scene.metadata),
                scenes::owner_id.eq(new_scene.owner_id),
                scenes::created_at.eq(new_scene.created_at),
                scenes::updated_at.eq(new_scene.updated_at),
            );

            diesel::insert_into(scenes::table)
                .values(values)
                .returning(crate::models::Scene::as_returning())
                .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to create scene"))?;

        Ok(GraphQLScene::from(inserted_scene))
    }

    async fn update_scene(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
        input: GraphQLUpdateSceneInput,
    ) -> GraphQLResult<GraphQLScene> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let is_admin = auth_user.is_admin;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let _now = Utc::now().naive_utc();

        let updated_scene = tokio::task::spawn_blocking(move || {
            use crate::schema::scenes;
            use diesel::prelude::*;

            // 🔐 A scene is content, so editing one follows the world role:
            // the Owner and any GM, not just whoever created the scene.
            // `updateSceneHidden` and `launchScene` next door already gate
            // this way — a GM who could hide and launch a scene but not
            // rename it was the inconsistency, not the fix.
            if !crate::auth::world_membership::is_dm_of_scene(
                &mut conn, user_id, is_admin, scene_id,
            )? {
                return Err(diesel::result::Error::NotFound);
            }

            // Spec 022 (FR-006): summaryRenderedHtml is derived from
            // summaryMarkdown at write time (not on read, unlike lore
            // entries — scenes have no `[[link]]` resolution need, so
            // there's no staleness concern to justify computing it lazily).
            let summary_rendered_html = input
                .summary_markdown
                .as_deref()
                .map(crate::markdown::render_to_safe_html);

            let update_data = crate::models::SceneUpdate {
                name: input.name,
                description: input.description,
                grid_size: input.grid_size,
                grid_type: input.grid_type,
                width: input.width,
                height: input.height,
                metadata: input.metadata.map(|j| j.0),
                summary_markdown: input.summary_markdown,
                summary_rendered_html,
                hidden: None,
                preview_asset_id: None,
            };

            diesel::update(scenes::table.filter(scenes::scene_id.eq(scene_id)))
                .set(update_data)
                .returning(crate::models::Scene::as_returning())
                .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to update scene"))?;

        Ok(GraphQLScene::from(updated_scene))
    }

    /// Spec 022 (FR-007, FR-019). See `update_scene_hidden_impl`.
    async fn update_scene_hidden(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
        hidden: bool,
    ) -> GraphQLResult<GraphQLScene> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        update_scene_hidden_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            scene_id,
            hidden,
        )
        .await
    }

    /// Spec 022 (FR-002a/FR-002b/FR-002c, ADR-046). See `launch_scene_impl`.
    async fn launch_scene(
        &self,
        ctx: &Context<'_>,
        world_id: uuid::Uuid,
        scene_id: uuid::Uuid,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        launch_scene_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            scene_id,
        )
        .await
    }

    async fn delete_scene(&self, ctx: &Context<'_>, scene_id: uuid::Uuid) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let is_admin = auth_user.is_admin;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::scenes;
            use diesel::prelude::*;

            // 🔐 Deleting a *scene* is a content act and follows the world
            // role. Deleting the *world* — and every other world-level
            // right — stays Owner-only and is gated elsewhere; a GM gains
            // nothing here beyond authority over the content of the world
            // they are running.
            if !crate::auth::world_membership::is_dm_of_scene(
                &mut conn, user_id, is_admin, scene_id,
            )? {
                // Same answer an unauthorized caller got before: nothing
                // was deleted.
                return Ok(0);
            }

            diesel::delete(scenes::table.filter(scenes::scene_id.eq(scene_id))).execute(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to delete scene"))?;

        Ok(deleted > 0)
    }

    // NOTE: scene-scoped token mutations (create_token/update_token/delete_token) live in
    // `mutations_tokens::TokenMutation` — that replaces an earlier `upsert_token`/`delete_token`
    // pair that lived here with no scene-ownership check at all. See TokenMutation for the
    // ownership-enforced, NOTIFY-synced replacement.

    async fn update_fog_mask(
        &self,
        ctx: &Context<'_>,
        input: GraphQLUpdateFogMaskInput,
    ) -> GraphQLResult<GraphQLFogMask> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;

        // Fog is what a Game Master uses to decide what a table can see, and
        // it had **no membership check at all** — any signed-in user could
        // write a mask onto any scene by naming its id. Reveal is the
        // dangerous direction: an attacker could uncover a map the GM was
        // deliberately keeping hidden, which is a spoiler at best and, on a
        // scene built around a secret, the whole session.
        let scene_id = input.scene_id;
        let is_admin = auth_user.is_admin;
        {
            let mut gate = state
                .db_pool
                .get()
                .map_err(|_| Error::new("Failed to get DB connection"))?;
            let permitted = tokio::task::spawn_blocking(move || {
                crate::auth::world_membership::is_dm_of_scene(
                    &mut gate, user_id, is_admin, scene_id,
                )
            })
            .await
            .map_err(|_| Error::new("Failed to spawn blocking task"))?
            .unwrap_or(false);
            if !permitted {
                return Err(Error::new("Forbidden"));
            }
        }

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let scene_id = input.scene_id;
        let bitmap_data_base64 = input.bitmap_data_base64;
        let width = input.width;
        let height = input.height;

        let updated_fog_mask = tokio::task::spawn_blocking(move || {
            use crate::schema::fog_masks;
            use diesel::prelude::*;

            let bitmap_bytes = base64::engine::general_purpose::STANDARD
                .decode(&bitmap_data_base64)
                .map_err(|_| DieselError::NotFound)?;

            diesel::insert_into(fog_masks::table)
                .values((
                    fog_masks::fog_id.eq(uuid::Uuid::now_v7()),
                    fog_masks::scene_id.eq(scene_id),
                    fog_masks::bitmap_data.eq(&bitmap_bytes),
                    fog_masks::version.eq(1),
                    fog_masks::width.eq(width),
                    fog_masks::height.eq(height),
                    fog_masks::updated_by.eq(user_id),
                    fog_masks::created_at.eq(now),
                    fog_masks::updated_at.eq(now),
                ))
                .on_conflict(fog_masks::scene_id)
                .do_update()
                .set((
                    fog_masks::bitmap_data.eq(&bitmap_bytes),
                    fog_masks::version.eq(fog_masks::version + 1),
                    fog_masks::width.eq(width),
                    fog_masks::height.eq(height),
                    fog_masks::updated_by.eq(user_id),
                    fog_masks::updated_at.eq(now),
                ))
                .returning(crate::models::FogMask::as_returning())
                .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to update fog mask"))?;

        Ok(GraphQLFogMask::from(updated_fog_mask))
    }
}

// Query structs moved to queries modules (Phase 4.9.Z Step 5)
