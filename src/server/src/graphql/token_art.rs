//! Playtest 2026-09-10 P1: a token shows its character's art.
//!
//! The engine draws exactly one image for a token, `tokens.photo_url`, and
//! nothing ever put a character's `token` image there — so every token placed
//! for a character was a flat coloured square. Rather than copy the art into
//! each token row when it is created (which every creation path would have to
//! remember, and which goes stale the moment the art is replaced), a token
//! with no photo of its own **resolves** to its character's token image when
//! it is read. A photo set on the token itself still wins.
//!
//! The URL carries `.webp` because Bevy chooses an image loader by extension:
//! `/api/actor-assets/{id}` alone loads nothing. `assets_serve::actor` accepts
//! the suffix for that reason, and lets anyone who can see a scene carrying
//! the token read this one image (see `token_art_visible_sync` there).

use std::collections::HashMap;

use diesel::prelude::*;
use uuid::Uuid;

use crate::graphql::GraphQLToken;
use crate::graphql::mutations_actor_images::ROLE_TOKEN;

/// Each actor's token image, as its asset id — one query for any number of
/// actors.
pub(crate) fn token_art_by_actor(
    conn: &mut PgConnection,
    actor_ids: &[Uuid],
) -> QueryResult<HashMap<Uuid, Uuid>> {
    use crate::schema::world_actor_images;

    if actor_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<(Uuid, Uuid)> = world_actor_images::table
        .filter(world_actor_images::actor_id.eq_any(actor_ids))
        .filter(world_actor_images::role.eq(ROLE_TOKEN))
        // Oldest first, so collecting keeps each actor's latest.
        .order(world_actor_images::updated_at.asc())
        .select((world_actor_images::actor_id, world_actor_images::asset_id))
        .load(conn)?;
    Ok(rows.into_iter().collect())
}

/// Where the engine loads a character's token image from.
pub(crate) fn token_art_url(asset_id: Uuid) -> String {
    format!("/api/actor-assets/{asset_id}.webp")
}

fn has_no_photo(photo_url: Option<&str>) -> bool {
    photo_url.is_none_or(str::is_empty)
}

/// Tokens as GraphQL, each one without a photo of its own given its
/// character's token art.
pub(crate) fn tokens_with_art(
    conn: &mut PgConnection,
    tokens: Vec<crate::models::Token>,
) -> QueryResult<Vec<GraphQLToken>> {
    let wanting: Vec<Uuid> = tokens
        .iter()
        .filter(|token| has_no_photo(token.photo_url.as_deref()))
        .filter_map(|token| token.actor_id)
        .collect();
    let art = token_art_by_actor(conn, &wanting)?;

    Ok(tokens
        .into_iter()
        .map(|token| {
            let fallback = token
                .actor_id
                .and_then(|actor_id| art.get(&actor_id))
                .map(|asset_id| token_art_url(*asset_id));
            GraphQLToken::from(token).with_photo_fallback(fallback)
        })
        .collect())
}

/// One token, as GraphQL, with its character's art — for the mutations that
/// hand a token back, so their answer agrees with the next read.
pub(crate) async fn token_with_art(
    state: &crate::state::AppState,
    token: crate::models::Token,
) -> async_graphql::Result<GraphQLToken> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || tokens_with_art(&mut conn, vec![token]))
        .await
        .map_err(|_| async_graphql::Error::new("Failed to spawn blocking task"))?
        .map_err(|_| async_graphql::Error::new("Failed to load the token's art"))?
        .pop()
        .ok_or_else(|| async_graphql::Error::new("Failed to load the token's art"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_url_names_its_format_so_the_engine_can_pick_a_loader() {
        let id = Uuid::now_v7();
        assert!(token_art_url(id).ends_with(".webp"));
        assert!(token_art_url(id).starts_with("/api/actor-assets/"));
    }

    #[test]
    fn an_empty_photo_is_no_photo() {
        assert!(has_no_photo(None));
        assert!(has_no_photo(Some("")));
        assert!(!has_no_photo(Some("/api/canvas-assets/x.webp")));
    }
}
