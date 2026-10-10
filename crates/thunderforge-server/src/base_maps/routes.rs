//! Spec 088 (US2, FR-021): the base maps' pictures and notice over HTTP.
//!
//! | Route | Answer |
//! | --- | --- |
//! | `GET /base-maps/<id>.thumb.webp` | the thumbnail |
//! | `GET /base-maps/<id>.webp` | the full image |
//! | `GET /base-maps/NOTICE.txt` | the licence notice |
//! | anything else | 404 |
//!
//! Mounted under `/api`, behind the signed-in check. The file name is
//! matched against the loaded set and never joined into a path, so `..`
//! cannot leave the directory. The pictures never change while the server
//! runs, so they are cached for a year.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use super::{BaseMaps, Picture};
use crate::state::AppState;

pub const IMMUTABLE: &str = "public, max-age=31536000, immutable";

pub fn router() -> Router<AppState> {
    Router::new().route("/base-maps/{file}", get(serve))
}

/// What a file name under `/base-maps/` names, if anything.
#[derive(Debug, PartialEq, Eq)]
pub enum Requested<'a> {
    Picture(&'a str, Picture),
    Notice,
}

pub fn requested(file: &str) -> Option<Requested<'_>> {
    if file == "NOTICE.txt" {
        return Some(Requested::Notice);
    }
    if let Some(id) = file.strip_suffix(".thumb.webp") {
        return Some(Requested::Picture(id, Picture::Thumbnail));
    }
    file.strip_suffix(".webp")
        .map(|id| Requested::Picture(id, Picture::Full))
}

async fn serve(State(state): State<AppState>, Path(file): Path<String>) -> Response {
    answer(&state.base_maps, &file).await
}

pub async fn answer(maps: &BaseMaps, file: &str) -> Response {
    let (path, content_type, cache) = match requested(file) {
        Some(Requested::Picture(id, picture)) => match maps.picture_path(id, picture) {
            Some(path) => (path, "image/webp", IMMUTABLE),
            None => return StatusCode::NOT_FOUND.into_response(),
        },
        Some(Requested::Notice) => match maps.notice_path() {
            Some(path) => (path, "text/plain; charset=utf-8", "public, max-age=3600"),
            None => return StatusCode::NOT_FOUND.into_response(),
        },
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, content_type),
                (header::CACHE_CONTROL, cache),
            ],
            bytes,
        )
            .into_response(),
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "base map file unreadable");
            StatusCode::NOT_FOUND.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base_maps::{BaseMap, MapCredit};

    fn credit() -> MapCredit {
        MapCredit {
            author: "MBRound18".into(),
            licence: "CC BY-SA 4.0".into(),
            licence_url: "https://creativecommons.org/licenses/by-sa/4.0/".into(),
            source: "https://github.com/mbround18/vtt-maps".into(),
            catalog: "https://vtt-maps.dnd-apps.dev/catalog".into(),
        }
    }

    fn one_map(dir: &std::path::Path) -> BaseMaps {
        std::fs::write(dir.join("forge.webp"), b"FULL").unwrap();
        std::fs::write(dir.join("forge.thumb.webp"), b"THUMB").unwrap();
        std::fs::write(dir.join("NOTICE.txt"), b"NOTICE").unwrap();
        std::fs::write(dir.join("secret.webp"), b"NOT LISTED").unwrap();
        BaseMaps::for_tests(
            dir.to_path_buf(),
            vec![BaseMap {
                id: "forge".into(),
                name: "Forge".into(),
                width: 10,
                height: 10,
                grid_size: 5,
                ambient_light: "bright".into(),
                walls: vec![],
                lights: vec![],
                has_thumbnail: true,
            }],
            credit(),
        )
    }

    async fn body(response: Response) -> Vec<u8> {
        axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec()
    }

    #[tokio::test]
    async fn a_known_map_is_served_with_a_year_long_cache() {
        let dir = tempfile::tempdir().unwrap();
        let maps = one_map(dir.path());

        let full = answer(&maps, "forge.webp").await;
        assert_eq!(full.status(), StatusCode::OK);
        assert_eq!(full.headers()[header::CACHE_CONTROL], IMMUTABLE);
        assert_eq!(full.headers()[header::CONTENT_TYPE], "image/webp");
        assert_eq!(body(full).await, b"FULL");

        let thumb = answer(&maps, "forge.thumb.webp").await;
        assert_eq!(thumb.headers()[header::CACHE_CONTROL], IMMUTABLE);
        assert_eq!(body(thumb).await, b"THUMB");

        let notice = answer(&maps, "NOTICE.txt").await;
        assert_eq!(notice.status(), StatusCode::OK);
        assert_eq!(body(notice).await, b"NOTICE");
    }

    #[tokio::test]
    async fn an_unknown_id_a_path_and_an_unlisted_file_are_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let maps = one_map(dir.path());
        for file in [
            "secret.webp",
            "..webp",
            "../forge.webp",
            "..%2Fforge.webp",
            "maps.json",
            "credit.json",
            "forge.png",
            "forge",
        ] {
            assert_eq!(
                answer(&maps, file).await.status(),
                StatusCode::NOT_FOUND,
                "{file}"
            );
        }
    }

    #[tokio::test]
    async fn with_no_maps_even_the_notice_is_not_found() {
        let maps = BaseMaps::default();
        assert_eq!(
            answer(&maps, "NOTICE.txt").await.status(),
            StatusCode::NOT_FOUND
        );
    }
}
