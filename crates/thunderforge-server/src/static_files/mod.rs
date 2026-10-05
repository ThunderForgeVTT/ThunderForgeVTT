//! Unauthenticated static-file mounts.
//!
//! The asset directory, and the built web client. Nothing here looks at who
//! is asking, which is why it no longer shares the word `serve` with
//! [`crate::assets_serve`] — that module is entirely permission checks that
//! happen to end in bytes, and the two being named alike invited exactly the
//! wrong assumption about this one.
//!
//! # One process serves the whole app
//!
//! The client is a directory beside the binary, named at runtime by
//! `--static-dir` / `STATIC_DIR`, never bytes inside it. Two things used to
//! stop the server from serving it, and a second nginx image stood in front
//! to cover for both:
//!
//! 1. The bundle asks for `/assets/entry/*.js`, `/assets/chunks/*.js` and
//!    `/assets/static/*.css`, and `/assets` was the upload directory alone, so
//!    every script and stylesheet was a 404. `/assets` now tries the bundle
//!    first and the uploads second.
//! 2. The client routes in the browser, so `/world/abc/play` is no file. Any
//!    path that is neither a file nor under `/api` now answers with the
//!    client's `index.html`.
//!
//! A server with no client built — every development and test run, where vite
//! serves the frontend — has no `index.html` to hand out, and answers an
//! unknown path with the JSON 404 it always has.

use crate::config::Directories;
use crate::errors::handler_404;
use crate::settings::features::{DEMO, flag_on};
use crate::state::AppState;
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Router, routing::get_service};
use std::path::Path;
use tower_http::services::{ServeDir, ServeFile};

pub fn router<S>(directories: &Directories) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let client = Path::new(&directories.static_files);
    let index = client.join("index.html");

    let router = Router::new().nest_service(
        "/assets",
        get_service(
            ServeDir::new(client.join("assets"))
                .fallback(ServeDir::new(&directories.asset_directory)),
        ),
    );

    if index.is_file() {
        router.fallback_service(get_service(
            ServeDir::new(client)
                .append_index_html_on_directories(true)
                .fallback(ServeFile::new(index)),
        ))
    } else {
        router.fallback(handler_404)
    }
}

/// The demo (spec 074), at `/demo`.
///
/// A second client, built apart from the first and served the same way: its
/// files, and its own `index.html` for any path under `/demo` that is not
/// one. A server started without `--demo-dir`, or pointed at a directory with
/// nothing built in it, mounts nothing, and `/demo` falls to whatever answers
/// every other unknown path.
///
/// Whether the instance offers it is not decided here; see
/// [`require_demo_offered`].
pub fn demo_router<S>(directories: &Directories) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let Some(demo) = directories.demo_files.as_deref().map(Path::new) else {
        return Router::new();
    };
    let index = demo.join("index.html");
    if !index.is_file() {
        return Router::new();
    }
    Router::new().nest_service(
        "/demo",
        get_service(
            ServeDir::new(demo)
                .append_index_html_on_directories(true)
                .fallback(ServeFile::new(index)),
        ),
    )
}

/// Answers "not found" for the demo while the instance does not offer it
/// (spec 074 FR-015).
///
/// Asked per request, because the flag is a setting and an administrator
/// switching it must not need a restart. Off is the default and the answer
/// when the setting cannot be read.
pub async fn require_demo_offered(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if flag_on(&state, DEMO).await.unwrap_or(false) {
        next.run(request).await
    } else {
        handler_404().await.into_response()
    }
}

#[cfg(test)]
mod tests;
