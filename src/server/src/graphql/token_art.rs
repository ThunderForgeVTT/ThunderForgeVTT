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
//!
//! # And its name (playtest 2026-09-10 P7)
//!
//! Resolved the same way and in the same place: the token's own label, else
//! its character's name. And disclosed here, because every token a client is
//! sent passes through this module — the scene's token list and every
//! mutation's answer — so a name a Game Master hid from players is withheld
//! from all of them in one place (`GraphQLToken::for_viewer`). Token events
//! carry ids only, so the broadcast never needs the same care.

use std::collections::HashMap;

use diesel::prelude::*;
use uuid::Uuid;

use crate::graphql::GraphQLToken;
use crate::graphql::mutations_actor_images::ROLE_TOKEN;
use crate::graphql::types_scene::written_label;

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

/// Each actor's name — one query for any number of actors.
fn character_names(
    conn: &mut PgConnection,
    actor_ids: &[Uuid],
) -> QueryResult<HashMap<Uuid, String>> {
    use crate::schema::world_actors;

    if actor_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<(Uuid, String)> = world_actors::table
        .filter(world_actors::id.eq_any(actor_ids))
        .select((world_actors::id, world_actors::label))
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

/// Tokens as GraphQL, as this viewer may see them: each one without a photo
/// of its own given its character's token art, each one without a name of its
/// own given its character's name, and a name hidden from players withheld
/// unless the viewer runs the world.
pub(crate) fn tokens_with_art(
    conn: &mut PgConnection,
    tokens: Vec<crate::models::Token>,
    viewer_runs_the_world: bool,
) -> QueryResult<Vec<GraphQLToken>> {
    let wanting_art: Vec<Uuid> = tokens
        .iter()
        .filter(|token| has_no_photo(token.photo_url.as_deref()))
        .filter_map(|token| token.actor_id)
        .collect();
    let art = token_art_by_actor(conn, &wanting_art)?;

    let wanting_name: Vec<Uuid> = tokens
        .iter()
        .filter(|token| written_label(token.metadata.as_ref()).is_none())
        .filter_map(|token| token.actor_id)
        .collect();
    let names = character_names(conn, &wanting_name)?;

    Ok(tokens
        .into_iter()
        .map(|token| {
            let actor = token.actor_id;
            let fallback_art = actor
                .and_then(|actor_id| art.get(&actor_id))
                .map(|asset_id| token_art_url(*asset_id));
            let fallback_name = actor.and_then(|actor_id| names.get(&actor_id).cloned());
            GraphQLToken::from(token)
                .with_photo_fallback(fallback_art)
                .with_name_fallback(fallback_name)
                .for_viewer(viewer_runs_the_world)
        })
        .collect())
}

/// One token, as GraphQL, as `user_id` may see it — for the mutations that
/// hand a token back, so their answer agrees with the next read. Two of them
/// are a player's own (`moveOwnToken`, `setOwnPrimaryTokenPhoto`), which is
/// why the viewer is worked out here rather than assumed.
pub(crate) async fn token_with_art(
    state: &crate::state::AppState,
    token: crate::models::Token,
    user_id: Uuid,
    is_admin: bool,
) -> async_graphql::Result<GraphQLToken> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || {
        let runs_the_world = crate::auth::world_membership::is_dm_of_scene(
            &mut conn,
            user_id,
            is_admin,
            token.scene_id,
        )?;
        tokens_with_art(&mut conn, vec![token], runs_the_world)
    })
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

    fn token(label: Option<&str>, visible: bool) -> crate::models::Token {
        let now = chrono::Utc::now().naive_utc();
        crate::models::Token {
            token_id: Uuid::now_v7(),
            scene_id: Uuid::now_v7(),
            actor_id: None,
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            scale: 1.0,
            metadata: label.map(|l| serde_json::json!({ "label": l, "note": "kept" })),
            created_at: now,
            updated_at: now,
            owner_user_id: None,
            is_primary: false,
            photo_url: None,
            health: None,
            max_health: None,
            token_type: "npc".to_string(),
            name_visible_to_players: visible,
        }
    }

    /// The whole token as a client would receive it, as JSON text — so a
    /// test asks the question that matters: does the name appear anywhere.
    fn as_sent(token: &GraphQLToken) -> String {
        format!("{token:?}")
    }

    #[test]
    fn a_hidden_name_reaches_a_game_master_and_nobody_else() {
        let gm = GraphQLToken::from(token(Some("The Lich"), false)).for_viewer(true);
        assert!(as_sent(&gm).contains("The Lich"));

        let player = GraphQLToken::from(token(Some("The Lich"), false)).for_viewer(false);
        assert!(
            !as_sent(&player).contains("The Lich"),
            "not as the name, and not in the metadata it was written in: {player:?}"
        );
        assert!(
            as_sent(&player).contains("kept"),
            "the rest of the metadata stays"
        );
    }

    #[test]
    fn a_shown_name_reaches_everyone() {
        let player = GraphQLToken::from(token(Some("Grom"), true)).for_viewer(false);
        assert!(as_sent(&player).contains("Grom"));
    }

    #[test]
    fn a_token_without_a_name_is_called_by_its_characters() {
        let named = GraphQLToken::from(token(None, true))
            .with_name_fallback(Some("Sir Pip".to_string()))
            .for_viewer(false);
        assert!(as_sent(&named).contains("Sir Pip"));

        // Its own name wins; a hidden fallback is withheld like any other.
        let own = GraphQLToken::from(token(Some("Pip"), true))
            .with_name_fallback(Some("Sir Pip".to_string()));
        assert!(as_sent(&own).contains("Some(\"Pip\")"));
        let hidden = GraphQLToken::from(token(None, false))
            .with_name_fallback(Some("Sir Pip".to_string()))
            .for_viewer(false);
        assert!(!as_sent(&hidden).contains("Sir Pip"));
    }
}
