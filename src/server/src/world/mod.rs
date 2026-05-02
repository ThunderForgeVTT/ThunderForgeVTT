use crate::state::AppState;
use axum::{
    middleware::{self, from_fn_with_state},
    Json, Router,
    extract::{Path, State},
    response::{
        IntoResponse,
        sse::{Event, Sse},
    },
    routing::{get, post},
};
use futures_util::stream::{Stream, StreamExt};
use std::convert::Infallible;
use thunderforge_core::events::WorldEvent;
use tokio_stream::wrappers::BroadcastStream;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/world/all", get(all_worlds))
        .route("/world/:id/events", get(world_events_by_id))
        .route("/world/:id/event", post(world_event_by_id))
        .route_layer(from_fn_with_state(
            AppState {
                config: crate::config::Config::default(),
                directories: crate::config::Directories::from(String::new()),
                world_event_sender: tokio::sync::broadcast::channel(1).0,
                key: tower_cookies::Key::generate(),
                db_pool: {
                    let manager = diesel::r2d2::ConnectionManager::<diesel::pg::PgConnection>::new("");
                    diesel::r2d2::Pool::builder().max_size(1).build_unchecked(manager)
                },
            },
            crate::auth_middleware::require_authenticated_user,
        ))
}

async fn all_worlds() -> impl IntoResponse {
    Json(vec![] as Vec<String>)
}

async fn world_events_by_id(
    Path(_id): Path<String>,
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.world_event_sender.subscribe();
    let stream = BroadcastStream::new(rx)
        .filter_map(|msg| async { msg.ok() })
        .map(|msg| Ok(Event::default().json_data(msg).unwrap()));

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}

async fn world_event_by_id(
    Path(_id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<WorldEvent>,
) {
    let _ = state.world_event_sender.send(payload);
}
