//! Small world REST endpoints.
//!
//! # Two routes were removed from here, and why
//!
//! `GET /world/{id}/events` (SSE) and `POST /world/{id}/event` used to live
//! beside `all_worlds`. Both were unreachable from the client — there is no
//! `EventSource` anywhere in `apps/web` and nothing referenced either path —
//! and both were unauthorized in a way that made them worth deleting rather
//! than repairing:
//!
//! - The SSE route bound `Extension(_auth_user)` and `Path(_id)` and then
//!   used **neither**. It subscribed to the process-wide event channel and
//!   streamed it verbatim, so any authenticated user could open it with any
//!   world id and receive a live feed of **every world's events in the
//!   system** — worlds they had never joined, worlds they had been revoked
//!   from, and the ids of scenes, chat messages and combats inside them.
//!   Bodies were not exposed (this bus carries nudges, not content), but who
//!   is doing what, where and when was.
//! - The POST route deserialized a client-supplied `WorldEvent` and pushed it
//!   straight onto that same channel — no membership check, no world check,
//!   no validation, and no database row. It let any authenticated user forge
//!   arbitrary events into any world, delivered live to that world's real
//!   subscribers.
//!
//! Together they were a cross-tenant read and write on dead code. Deleting
//! them removes the surface; the supported path for world events is the
//! `worldEventsCreated` GraphQL subscription, which authorizes world
//! membership and filters by world.

use crate::auth_middleware::AuthenticatedUser;
use crate::models::World;
use crate::schema::worlds;
use crate::state::AppState;
use axum::{
    Json, Router,
    extract::{Extension, State},
    response::IntoResponse,
    routing::get,
};
use diesel::prelude::*;

pub fn router() -> Router<AppState> {
    Router::new().route("/world/all", get(all_worlds))
}

async fn all_worlds(
    Extension(auth_user): Extension<AuthenticatedUser>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let user_id = auth_user.user_id;
    let mut conn = match state.db_pool.get() {
        Ok(conn) => conn,
        Err(_) => return Json(Vec::<String>::new()),
    };

    let names = tokio::task::spawn_blocking(move || {
        worlds::table
            .filter(worlds::created_by.eq(user_id))
            .order(worlds::created_at.desc())
            .select(World::as_select())
            .load::<World>(&mut conn)
            .map(|items| {
                items
                    .into_iter()
                    .map(|world| world.name)
                    .collect::<Vec<_>>()
            })
    })
    .await
    .ok()
    .and_then(Result::ok)
    .unwrap_or_default();

    Json(names)
}
