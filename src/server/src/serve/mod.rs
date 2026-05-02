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
