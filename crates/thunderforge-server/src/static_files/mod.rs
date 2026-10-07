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
//!
//! # Kept until its name changes
//!
//! Every file the bundle builds into `assets/` carries a hash of its bytes in
//! its name (`engine_bg-D9SKK7e3.wasm`), so a file at a given name never
//! changes: a new engine is a new name. Those are sent as kept forever, and a
//! visitor downloads the engine once, not once per visit. The page that names
//! them is sent as `no-cache`, so a deploy is seen on the next load. Uploads
//! share `/assets` but keep their names across edits, so they are left alone.

use crate::config::Directories;
use crate::errors::handler_404;
use crate::settings::features::{DEMO, flag_on};
use crate::state::AppState;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Router, middleware::map_response, routing::get_service};
use std::path::Path;
use tower_http::services::{ServeDir, ServeFile};

pub fn router<S>(directories: &Directories) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let client = Path::new(&directories.static_files);
    let index = client.join("index.html");

    let mut router = Router::new().nest_service(
        "/assets",
        get_service(
            ServeDir::new(client.join("assets"))
                .fallback(ServeDir::new(&directories.asset_directory)),
        ),
    );
    // The folders the bundle builds into; no upload is ever written there.
    for folder in ["entry", "chunks", "static"] {
        router = router.nest_service(
            &format!("/assets/{folder}"),
            get_service(ServeDir::new(client.join("assets").join(folder)))
                .layer(map_response(kept_forever)),
        );
    }

    if index.is_file() {
        router.fallback_service(
            get_service(
                ServeDir::new(client)
                    .append_index_html_on_directories(true)
                    .fallback(ServeFile::new(index)),
            )
            .layer(map_response(asked_every_time)),
        )
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
    Router::new()
        .nest_service(
            "/demo/assets",
            get_service(ServeDir::new(demo.join("assets"))).layer(map_response(kept_forever)),
        )
        .nest_service(
            "/demo",
            get_service(
                ServeDir::new(demo)
                    .append_index_html_on_directories(true)
                    .fallback(ServeFile::new(index)),
            )
            .layer(map_response(asked_every_time)),
        )
}

/// A built file, named by its own hash: kept until a build renames it. A miss
/// is not kept, because the next build may put a file at that name.
async fn kept_forever(mut response: Response) -> Response {
    if response.status().is_success() {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    }
    response
}

/// The page that names the built files, and the files that keep their names
/// across builds (the maps, `robots.txt`): asked for again on every load, so a
/// deploy is seen at once. Unchanged, the answer is a cheap 304.
async fn asked_every_time(mut response: Response) -> Response {
    response
        .headers_mut()
        .entry(header::CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("no-cache"));
    response
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
