//! Unauthenticated static-file mounts.
//!
//! Two `ServeDir`s: the asset directory, and the built web client as the
//! fallback. Nothing here looks at who is asking, which is why it no longer
//! shares the word `serve` with [`crate::assets_serve`] — that module is
//! entirely permission checks that happen to end in bytes, and the two being
//! named alike invited exactly the wrong assumption about this one.

use crate::config::Directories;
use crate::state::AppState;
use axum::{Router, routing::get_service};
use tower_http::services::ServeDir;

pub fn router(directories: &Directories) -> Router<AppState> {
    Router::new()
        .nest_service(
            "/assets",
            get_service(ServeDir::new(&directories.asset_directory)),
        )
        .fallback_service(get_service(
            ServeDir::new(&directories.static_files).append_index_html_on_directories(true),
        ))
}
