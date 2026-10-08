//! `shapeCreators(sceneId)` — who drew on a scene, for the Game Master's
//! "clear a player's shapes" picker (spec 082 US4, research R6).

use async_graphql::{Context, Error, Object, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use uuid::Uuid;

use crate::auth::shape_authority::{ShapeAuthority, shape_authority};
use crate::graphql::{app_state, authenticated_user};
use crate::schema::{scenes, shapes, users, world_members, worlds};

/// One person who drew on a scene and does not run its world.
#[derive(SimpleObject, Debug, Clone, PartialEq)]
pub struct ShapeCreator {
    pub user_id: Uuid,
    /// `users.username`, as the grants card shows it.
    pub display_name: String,
    /// False once they left or were removed from the world.
    pub is_member: bool,
    /// Their shapes on this scene, every level.
    pub shape_count: i32,
}

#[derive(Default)]
pub struct ShapeCreatorsQuery;

#[Object]
impl ShapeCreatorsQuery {
    /// Each creator of a shape on the scene who is not an Owner or GM of its
    /// world today, by display name. A Game Master of the scene only; anyone
    /// else is told the scene was not found.
    async fn shape_creators(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
    ) -> GraphQLResult<Vec<ShapeCreator>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let is_admin = auth_user.is_admin;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        tokio::task::spawn_blocking(move || {
            if shape_authority(&mut conn, user_id, is_admin, scene_id, None)? != ShapeAuthority::Dm
            {
                return Err(DieselError::NotFound);
            }
            creators_on(&mut conn, scene_id)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|e| match e {
            DieselError::NotFound => Error::new("Scene not found"),
            _ => Error::new("Failed to list shape creators"),
        })
    }
}

fn creators_on(conn: &mut PgConnection, scene_id: Uuid) -> QueryResult<Vec<ShapeCreator>> {
    let world_id: Uuid = scenes::table
        .find(scene_id)
        .select(scenes::world_id)
        .first(conn)?;
    // `create_world` writes no member row for its creator, who is the
    // world's Owner all the same (`auth::world_membership`).
    let creator: Uuid = worlds::table
        .find(world_id)
        .select(worlds::created_by)
        .first(conn)?;
    let per_creator: Vec<(Uuid, i64)> = shapes::table
        .filter(shapes::scene_id.eq(scene_id))
        .group_by(shapes::created_by)
        .select((shapes::created_by, diesel::dsl::count_star()))
        .load(conn)?;
    let names: Vec<(Uuid, String)> = users::table
        .filter(users::id.eq_any(per_creator.iter().map(|c| c.0)))
        .select((users::id, users::username))
        .order(users::username)
        .load(conn)?;
    let counts = names.into_iter().filter_map(|(user_id, name)| {
        let count = per_creator.iter().find(|c| c.0 == user_id)?.1;
        Some((user_id, name, count))
    });
    let roles: Vec<(Uuid, String)> = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .filter(world_members::user_id.eq_any(per_creator.iter().map(|c| c.0)))
        .select((world_members::user_id, world_members::role))
        .load(conn)?;

    Ok(counts
        .filter_map(|(user_id, display_name, count)| {
            let role = roles.iter().find(|r| r.0 == user_id).map(|r| r.1.as_str());
            // Whoever runs the world today is not a "player's shapes" entry.
            if user_id == creator || matches!(role, Some("Owner" | "GM")) {
                return None;
            }
            Some(ShapeCreator {
                user_id,
                display_name,
                is_member: role.is_some(),
                shape_count: count as i32,
            })
        })
        .collect())
}

#[cfg(test)]
#[path = "shape_creators_tests.rs"]
mod tests;
