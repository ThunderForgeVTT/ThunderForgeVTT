use crate::state::AppState;
use crate::auth_middleware::AuthenticatedUser;
use axum::{
    Json, Router,
    extract::{Extension, Path, State},
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
}

async fn all_worlds(Extension(auth_user): Extension<AuthenticatedUser>) -> impl IntoResponse {
    let _ = (auth_user.user_id, auth_user.session_id);
    Json(vec![] as Vec<String>)
}

async fn world_events_by_id(
    Extension(_auth_user): Extension<AuthenticatedUser>,
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
    Extension(_auth_user): Extension<AuthenticatedUser>,
    Path(_id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<WorldEvent>,
) {
    let _ = state.world_event_sender.send(payload);
}
