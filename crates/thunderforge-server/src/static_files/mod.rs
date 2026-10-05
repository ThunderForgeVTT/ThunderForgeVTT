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

#[cfg(test)]
mod tests;
