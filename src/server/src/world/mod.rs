use crate::auth_middleware::AuthenticatedUser;
use crate::models::{World, WorldEvent};
use crate::schema::worlds;
use crate::state::AppState;
use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    response::{
        IntoResponse,
        sse::{Event, Sse},
    },
    routing::{get, post},
};
use diesel::prelude::*;
use futures_util::stream::{Stream, StreamExt};
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/world/all", get(all_worlds))
        .route("/world/{id}/events", get(world_events_by_id))
        .route("/world/{id}/event", post(world_event_by_id))
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

async fn world_events_by_id(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(_id): Path<String>,
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.world_event_sender.subscribe();
    // `msg.ok()` used to drop `Lagged(n)` without so much as a log — the
    // quietest of the three places this crate discards events. An SSE client
    // that falls behind loses n updates and is told nothing, by anyone.
    let stream = BroadcastStream::new(rx)
        .filter_map(|msg| async {
            match msg {
                Ok(event) => Some(event),
                Err(BroadcastStreamRecvError::Lagged(missed)) => {
                    eprintln!(
                        "[SSE] ⚠️  DROPPED {missed} event(s) for a subscriber: it fell behind                          the broadcast buffer. Those events will never be delivered to it."
                    );
                    None
                }
            }
        })
        .map(|msg| Ok(Event::default().json_data(msg).unwrap()));

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}

async fn world_event_by_id(
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(_id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<WorldEvent>,
) {
    let _ = state.world_event_sender.send(payload);
}
