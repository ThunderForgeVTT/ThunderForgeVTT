//! Spec 031 (T069, US8/FR-036): giving an actor a portrait and a token image.
//!
//! # Why two roles and not two columns
//!
//! ADR-057. A portrait is a face in a panel and a token is what stands on the
//! map; they are different pictures for different places, and the deferred
//! talking/not-talking/background set is more of the same. So imagery is rows
//! in `world_actor_images` keyed by `role`, unique on (`actor_id`, `role`) —
//! re-uploading a role replaces that role's image and touches no other.
//!
//! # Why the roles are constants and not an enum
//!
//! `role` is open text in the database (ADR-054's reasoning: a central list
//! every new role must edit is the coupling worth avoiding). The two names
//! this feature uses are declared here so the server and its tests spell them
//! the same way, and an unrecognised role is ignored rather than rendered.
//!
//! # Why the bytes take the lore path
//!
//! `mutations_lore_images.rs` is the pattern and the ordering is copied from
//! it verbatim: authorize → transcode to WebP renditions → write both objects
//! → persist the row, so an actor is never left pointing at an asset that was
//! never written. The alternative — routing actor imagery through
//! `uploadCanvasImage` — was rejected because a canvas asset is scoped to a
//! scene and its read is gated on that scene's visibility, which would make a
//! player's portrait vanish the moment a Game Master hid the scene the actor
//! happens to belong to. An actor's picture is not scene knowledge.
//!
//! Reading them back is `assets_serve/actor.rs`, which mirrors
//! `assets_serve/lore.rs` in the same way.

use async_graphql::{Context, Error, ErrorExtensions, Result as GraphQLResult, Upload};
use diesel::prelude::*;
use uuid::Uuid;

use crate::assets_serve::actor::{actor_image_full_key, actor_image_thumb_key};
use crate::auth::actor_permissions::require_actor_permission;
use crate::graphql::types::{ActorPermissionLevel, GraphQLActorImage};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{NewWorldActorImage, WorldActorImage};
use crate::schema::world_actor_images;
use crate::state::AppState;
use crate::storage::rustfs::{RustFsConfig, write_object};
use crate::storage::transcode::{TranscodeError, transcode_to_lore_renditions};

/// The character's face, for a sheet or a panel.
pub const ROLE_PORTRAIT: &str = "portrait";
/// What stands on the map.
pub const ROLE_TOKEN: &str = "token";

/// The roles this feature knows how to render.
///
/// Enforced at the mutation rather than in the schema: the column stays open
/// so a later role is additive, but nothing should be able to write a typo
/// into it through the only door that exists today.
pub const KNOWN_ROLES: [&str; 2] = [ROLE_PORTRAIT, ROLE_TOKEN];

#[derive(Debug, thiserror::Error)]
pub enum UploadActorImageError {
    #[error("insufficient permission to change this actor's imagery")]
    Forbidden,
    #[error("unknown image role '{0}' — expected 'portrait' or 'token'")]
    UnknownRole(String),
    #[error("upload exceeds maximum size of {max} bytes (got {actual})")]
    TooLarge { max: usize, actual: usize },
    #[error("failed to decode/transcode image: {0}")]
    Transcode(String),
    #[error("failed to write object to storage: {0}")]
    Storage(String),
    #[error("database error: {0}")]
    Database(String),
}

fn to_graphql_error(e: UploadActorImageError) -> Error {
    let msg = e.to_string();
    if matches!(e, UploadActorImageError::Forbidden) {
        Error::new(msg).extend_with(|_, ext| ext.set("code", "FORBIDDEN"))
    } else {
        Error::new(msg)
    }
}

/// FR-036. Ordering mirrors `upload_lore_image_impl`: authorize (Editor or
/// Owner on the actor, the same gate `updateActor` uses — Constitution
/// Principle III puts the rule here, at the data boundary, not in whichever
/// screen happens to offer the button) → transcode both renditions, so an
/// oversized or undecodable upload is refused before anything is written →
/// write both objects → upsert the role's row last.
///
/// The upsert is what makes "replace the portrait" a single call: the unique
/// index on (`actor_id`, `role`) turns a second upload of the same role into
/// an update, so the caller never has to delete first and never risks two
/// rows racing for one role.
pub async fn upload_actor_image_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
    role: String,
    file_bytes: Vec<u8>,
) -> Result<WorldActorImage, UploadActorImageError> {
    if !KNOWN_ROLES.contains(&role.as_str()) {
        return Err(UploadActorImageError::UnknownRole(role));
    }

    require_actor_permission(
        state,
        user_id,
        is_admin,
        actor_id,
        ActorPermissionLevel::Editor,
    )
    .await
    .map_err(|_| UploadActorImageError::Forbidden)?;

    let renditions = transcode_to_lore_renditions(&file_bytes).map_err(|e| match e {
        TranscodeError::TooLarge { max, actual } => UploadActorImageError::TooLarge { max, actual },
        other => UploadActorImageError::Transcode(other.to_string()),
    })?;

    let asset_id = Uuid::now_v7();
    let cfg = RustFsConfig::from_env();
    write_object(
        &cfg,
        &actor_image_full_key(asset_id),
        renditions.full_webp_bytes,
        "image/webp",
    )
    .await
    .map_err(|e| UploadActorImageError::Storage(e.to_string()))?;
    write_object(
        &cfg,
        &actor_image_thumb_key(asset_id),
        renditions.thumbnail_webp_bytes,
        "image/webp",
    )
    .await
    .map_err(|e| UploadActorImageError::Storage(e.to_string()))?;

    let new_image = NewWorldActorImage {
        actor_id,
        role,
        asset_id,
        created_by: user_id,
        updated_by: user_id,
    };

    let mut conn = state
        .db_pool
        .get()
        .map_err(|e| UploadActorImageError::Database(e.to_string()))?;
    tokio::task::spawn_blocking(move || {
        diesel::insert_into(world_actor_images::table)
            .values(&new_image)
            .on_conflict((world_actor_images::actor_id, world_actor_images::role))
            .do_update()
            .set((
                world_actor_images::asset_id.eq(new_image.asset_id),
                world_actor_images::updated_by.eq(user_id),
                world_actor_images::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .returning(WorldActorImage::as_returning())
            .get_result::<WorldActorImage>(&mut conn)
    })
    .await
    .map_err(|e| UploadActorImageError::Database(e.to_string()))?
    .map_err(|e| UploadActorImageError::Database(e.to_string()))
}

/// Every image an actor has.
///
/// A query rather than a column-by-column read, which is the half of ADR-057's
/// bargain that pays for the join: a caller asks once and gets whatever roles
/// exist, including ones added after it was written.
pub async fn actor_images_impl(
    state: &AppState,
    actor_id: Uuid,
) -> GraphQLResult<Vec<WorldActorImage>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_actor_images::table
            .filter(world_actor_images::actor_id.eq(actor_id))
            .order(world_actor_images::role.asc())
            .select(WorldActorImage::as_select())
            .load::<WorldActorImage>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load actor images"))
}

/// Removes one role's image. Same Editor gate as setting it.
///
/// The stored object is deliberately left in place: nothing else in this
/// application deletes written bytes on a row delete either (`deleteLoreEntry`
/// keeps its images), and an orphan object costs storage where a delete that
/// races a read costs a broken image on someone's screen.
pub async fn remove_actor_image_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
    role: String,
) -> GraphQLResult<bool> {
    require_actor_permission(
        state,
        user_id,
        is_admin,
        actor_id,
        ActorPermissionLevel::Editor,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let deleted = tokio::task::spawn_blocking(move || {
        diesel::delete(
            world_actor_images::table
                .filter(world_actor_images::actor_id.eq(actor_id))
                .filter(world_actor_images::role.eq(role)),
        )
        .execute(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to remove actor image"))?;

    Ok(deleted > 0)
}

#[derive(Default)]
pub struct ActorImageMutation;

#[async_graphql::Object]
impl ActorImageMutation {
    async fn upload_actor_image(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
        role: String,
        file: Upload,
    ) -> GraphQLResult<GraphQLActorImage> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let upload_value = file.value(ctx).map_err(|e| Error::new(e.to_string()))?;
        let mut content = upload_value.into_read();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut content, &mut bytes)
            .map_err(|e| Error::new(format!("failed to read upload: {e}")))?;

        let image = upload_actor_image_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            actor_id,
            role,
            bytes,
        )
        .await
        .map_err(to_graphql_error)?;
        Ok(image.into())
    }

    async fn remove_actor_image(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
        role: String,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        remove_actor_image_impl(state, auth_user.user_id, auth_user.is_admin, actor_id, role).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_actors::{CreateActorInput, create_actor_impl};
    use crate::test_support::{
        insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
        test_app_state, tiny_png_bytes,
    };

    async fn actor_for_test(state: &AppState, owner_id: Uuid, world_id: Uuid) -> Uuid {
        create_actor_impl(
            state,
            owner_id,
            false,
            CreateActorInput {
                world_id,
                label: "Bandit".to_string(),
                is_npc: true,
                actor_type: None,
                game_system_id: None,
                description: None,
            },
        )
        .await
        .expect("world owner may create an actor")
        .id
    }

    /// FR-036: portrait and token are distinct rows on one actor, not one
    /// image overwriting the other.
    #[tokio::test]
    async fn upload_actor_image_keeps_roles_apart() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let actor_id = actor_for_test(&state, owner_id, world_id).await;

        let portrait = upload_actor_image_impl(
            &state,
            owner_id,
            false,
            actor_id,
            ROLE_PORTRAIT.to_string(),
            tiny_png_bytes(),
        )
        .await
        .expect("owner may upload a portrait");
        let token = upload_actor_image_impl(
            &state,
            owner_id,
            false,
            actor_id,
            ROLE_TOKEN.to_string(),
            tiny_png_bytes(),
        )
        .await
        .expect("owner may upload a token image");

        assert_ne!(
            portrait.asset_id, token.asset_id,
            "a portrait and a token are different pictures"
        );

        let images = actor_images_impl(&state, actor_id).await.unwrap();
        assert_eq!(images.len(), 2);
        let mut roles: Vec<&str> = images.iter().map(|i| i.role.as_str()).collect();
        roles.sort_unstable();
        assert_eq!(roles, vec![ROLE_PORTRAIT, ROLE_TOKEN]);
    }

    /// An actor has at most one image per role: re-uploading replaces.
    #[tokio::test]
    async fn upload_actor_image_replaces_the_same_role() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let actor_id = actor_for_test(&state, owner_id, world_id).await;

        let first = upload_actor_image_impl(
            &state,
            owner_id,
            false,
            actor_id,
            ROLE_PORTRAIT.to_string(),
            tiny_png_bytes(),
        )
        .await
        .unwrap();
        let second = upload_actor_image_impl(
            &state,
            owner_id,
            false,
            actor_id,
            ROLE_PORTRAIT.to_string(),
            tiny_png_bytes(),
        )
        .await
        .unwrap();

        assert_eq!(
            first.id, second.id,
            "the role's row is updated, not doubled"
        );
        assert_ne!(first.asset_id, second.asset_id);

        let mut conn = state.db_pool.get().unwrap();
        let count: i64 = world_actor_images::table
            .filter(world_actor_images::actor_id.eq(actor_id))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(count, 1);
    }

    /// Constitution Principle III: a Viewer-level member is refused at the
    /// data boundary, and no row survives the attempt.
    #[tokio::test]
    async fn upload_actor_image_rejects_viewer_level_caller() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        let viewer_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, viewer_id, "Player");
        drop(conn);

        let actor_id = actor_for_test(&state, owner_id, world_id).await;

        let result = upload_actor_image_impl(
            &state,
            viewer_id,
            false,
            actor_id,
            ROLE_PORTRAIT.to_string(),
            tiny_png_bytes(),
        )
        .await;
        assert!(matches!(result, Err(UploadActorImageError::Forbidden)));

        let mut conn = state.db_pool.get().unwrap();
        let count: i64 = world_actor_images::table
            .filter(world_actor_images::actor_id.eq(actor_id))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(count, 0, "a refused upload leaves no row behind");
    }

    /// A role nothing renders is refused rather than stored — the column is
    /// open for later roles, not for typos.
    #[tokio::test]
    async fn upload_actor_image_rejects_unknown_role() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let actor_id = actor_for_test(&state, owner_id, world_id).await;

        let result = upload_actor_image_impl(
            &state,
            owner_id,
            false,
            actor_id,
            "portrat".to_string(),
            tiny_png_bytes(),
        )
        .await;
        assert!(matches!(result, Err(UploadActorImageError::UnknownRole(_))));
    }

    /// Removing a role leaves the actor's other roles alone.
    #[tokio::test]
    async fn remove_actor_image_removes_only_that_role() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let actor_id = actor_for_test(&state, owner_id, world_id).await;
        for role in KNOWN_ROLES {
            upload_actor_image_impl(
                &state,
                owner_id,
                false,
                actor_id,
                role.to_string(),
                tiny_png_bytes(),
            )
            .await
            .unwrap();
        }

        let removed =
            remove_actor_image_impl(&state, owner_id, false, actor_id, ROLE_TOKEN.to_string())
                .await
                .unwrap();
        assert!(removed);

        let images = actor_images_impl(&state, actor_id).await.unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].role, ROLE_PORTRAIT);
    }
}
