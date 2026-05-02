use crate::state::AppState;
use axum::{
    extract::{Path, State},
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
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
