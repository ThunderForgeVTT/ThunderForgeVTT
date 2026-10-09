use super::{demo_router, router};
use crate::config::Directories;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::fs;
use std::path::Path;
use tower::ServiceExt;

/// A data directory with uploads, and a client directory apart from it.
fn directories(root: &Path) -> Directories {
    let data = root.join("data");
    fs::create_dir_all(data.join("assets")).unwrap();
    fs::write(data.join("assets").join("map.png"), "an upload").unwrap();
    Directories::from(data.to_str().unwrap().to_owned())
        .with_static_files(root.join("client").to_str().unwrap().to_owned())
}

fn build_client(root: &Path) {
    let client = root.join("client");
    fs::create_dir_all(client.join("assets").join("entry")).unwrap();
    fs::write(client.join("index.html"), "the client").unwrap();
    fs::write(client.join("favicon.svg"), "an icon").unwrap();
    fs::write(
        client.join("assets").join("entry").join("main.js"),
        "a script",
    )
    .unwrap();
}

async fn get(app: &Router, path: &str) -> (StatusCode, String) {
    let response = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn the_bundle_and_the_uploads_share_the_assets_path() {
    let root = tempfile::tempdir().unwrap();
    build_client(root.path());
    let app: Router = router(&directories(root.path()));

    assert_eq!(
        get(&app, "/assets/entry/main.js").await,
        (StatusCode::OK, "a script".to_owned())
    );
    assert_eq!(
        get(&app, "/assets/map.png").await,
        (StatusCode::OK, "an upload".to_owned())
    );
    assert_eq!(
        get(&app, "/assets/nothing.png").await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_path_the_client_routes_answers_with_the_client() {
    let root = tempfile::tempdir().unwrap();
    build_client(root.path());
    let app: Router = router(&directories(root.path()));

    for path in ["/", "/world/abc/play", "/setup/some-code"] {
        assert_eq!(
            get(&app, path).await,
            (StatusCode::OK, "the client".to_owned()),
            "{path}"
        );
    }
    assert_eq!(
        get(&app, "/favicon.svg").await,
        (StatusCode::OK, "an icon".to_owned())
    );
}

#[tokio::test]
async fn a_server_with_no_client_built_still_answers_not_found_as_json() {
    let root = tempfile::tempdir().unwrap();
    let app: Router = router(&directories(root.path()));

    let (status, body) = get(&app, "/world/abc/play").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains("Not Found"), "{body}");
    assert_eq!(
        get(&app, "/assets/map.png").await,
        (StatusCode::OK, "an upload".to_owned())
    );
}

fn build_demo(root: &Path) -> Directories {
    let demo = root.join("demo");
    fs::create_dir_all(demo.join("maps")).unwrap();
    fs::write(demo.join("index.html"), "the demo").unwrap();
    fs::write(demo.join("maps").join("NOTICE.txt"), "whose maps").unwrap();
    directories(root).with_demo_files(demo.to_str().unwrap().to_owned())
}

#[tokio::test]
async fn the_demo_is_a_second_client_under_its_own_path() {
    let root = tempfile::tempdir().unwrap();
    build_client(root.path());
    let directories = build_demo(root.path());
    let app: Router = demo_router(&directories).merge(router(&directories));

    for path in ["/demo", "/demo/", "/demo/world/demo/play"] {
        assert_eq!(
            get(&app, path).await,
            (StatusCode::OK, "the demo".to_owned()),
            "{path}"
        );
    }
    assert_eq!(
        get(&app, "/demo/maps/NOTICE.txt").await,
        (StatusCode::OK, "whose maps".to_owned())
    );
    // And the first client is still the first client.
    assert_eq!(
        get(&app, "/world/abc/play").await,
        (StatusCode::OK, "the client".to_owned())
    );
}

#[tokio::test]
async fn a_server_not_told_where_a_demo_is_has_none() {
    let root = tempfile::tempdir().unwrap();
    let app: Router =
        demo_router(&directories(root.path())).merge(router(&directories(root.path())));
    assert_eq!(get(&app, "/demo/").await.0, StatusCode::NOT_FOUND);

    // Named, but nothing built in it.
    let empty = directories(root.path())
        .with_demo_files(root.path().join("demo").to_str().unwrap().to_owned());
    let app: Router = demo_router(&empty).merge(router(&empty));
    assert_eq!(get(&app, "/demo/").await.0, StatusCode::NOT_FOUND);
}

async fn cache_control(app: &Router, path: &str) -> (StatusCode, Option<String>) {
    let response = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let header = response
        .headers()
        .get(axum::http::header::CACHE_CONTROL)
        .map(|value| value.to_str().unwrap().to_owned());
    (response.status(), header)
}

const FOREVER: &str = "public, max-age=31536000, immutable";

#[tokio::test]
async fn the_bundle_is_kept_until_its_name_changes() {
    let root = tempfile::tempdir().unwrap();
    build_client(root.path());
    let app: Router = router(&directories(root.path()));

    assert_eq!(
        cache_control(&app, "/assets/entry/main.js").await,
        (StatusCode::OK, Some(FOREVER.to_owned()))
    );
    // A miss is not kept: the next build may put the file there.
    assert_eq!(
        cache_control(&app, "/assets/entry/missing.js").await,
        (StatusCode::NOT_FOUND, None)
    );
    // An upload keeps its own name across edits, so it is never kept forever.
    assert_ne!(
        cache_control(&app, "/assets/map.png").await.1,
        Some(FOREVER.to_owned())
    );
}

#[tokio::test]
async fn the_page_that_names_the_bundle_is_asked_for_every_time() {
    let root = tempfile::tempdir().unwrap();
    build_client(root.path());
    let app: Router = router(&directories(root.path()));

    for path in ["/", "/world/abc/play"] {
        assert_eq!(
            cache_control(&app, path).await,
            (StatusCode::OK, Some("no-cache".to_owned())),
            "{path}"
        );
    }
}

#[tokio::test]
async fn the_demo_keeps_its_engine_and_asks_for_its_page() {
    let root = tempfile::tempdir().unwrap();
    let directories = build_demo(root.path());
    let demo = root.path().join("demo");
    fs::create_dir_all(demo.join("assets")).unwrap();
    fs::write(
        demo.join("assets").join("engine_bg-D9SKK7e3.wasm"),
        "an engine",
    )
    .unwrap();
    let app: Router = demo_router(&directories).merge(router(&directories));

    assert_eq!(
        cache_control(&app, "/demo/assets/engine_bg-D9SKK7e3.wasm").await,
        (StatusCode::OK, Some(FOREVER.to_owned()))
    );
    assert_eq!(
        cache_control(&app, "/demo/").await,
        (StatusCode::OK, Some("no-cache".to_owned()))
    );
    // The maps keep their names when they are re-imported.
    assert_ne!(
        cache_control(&app, "/demo/maps/NOTICE.txt").await.1,
        Some(FOREVER.to_owned())
    );
}

/// Spec 080 T020: a built file is sent as its precompressed copy to a client
/// that takes it, still offering parts and naming its version, and in parts
/// of the original to a client that asks for a range without compression.
#[tokio::test]
async fn a_built_file_goes_out_precompressed_and_still_in_parts() {
    let root = tempfile::tempdir().unwrap();
    build_client(root.path());
    let entry = root.path().join("client").join("assets").join("entry");
    fs::write(entry.join("main.js.br"), "brotli bytes").unwrap();
    fs::write(entry.join("main.js.gz"), "gzip bytes").unwrap();
    let app: Router = router(&directories(root.path()));

    let response = app
        .clone()
        .oneshot(
            Request::get("/assets/entry/main.js")
                .header("accept-encoding", "br, gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let h = response.headers();
    assert_eq!(h["content-encoding"], "br");
    assert_eq!(h["accept-ranges"], "bytes");
    assert!(h.get("last-modified").is_some(), "no version named");
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"brotli bytes");

    let response = app
        .clone()
        .oneshot(
            Request::get("/assets/entry/main.js")
                .header("range", "bytes=2-4")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert!(response.headers().get("content-encoding").is_none());
    assert_eq!(response.headers()["content-range"], "bytes 2-4/8");
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"scr");
}

/// An invitation link is a credential for joining the instance. The page's
/// own `<meta name="robots">` is added by script, which a crawler that does
/// not run scripts never sees, so the header says it on the response itself.
#[tokio::test]
async fn an_invitation_page_tells_crawlers_not_to_index_it() {
    let root = tempfile::tempdir().unwrap();
    build_client(root.path());
    let app: Router = router(&directories(root.path()));

    let robots = |response: &axum::response::Response| {
        response
            .headers()
            .get("x-robots-tag")
            .map(|value| value.to_str().unwrap().to_owned())
    };

    let response = app
        .clone()
        .oneshot(
            Request::get("/invite/CF289C5EC1C2487790FD")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(robots(&response), Some("noindex, nofollow".to_owned()));
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::CACHE_CONTROL)
            .map(|value| value.to_str().unwrap().to_owned()),
        Some("no-cache".to_owned())
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(body, "the client");

    // Every other page is left for the page itself to decide.
    let response = app
        .clone()
        .oneshot(Request::get("/world/abc/play").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(robots(&response), None);
}
