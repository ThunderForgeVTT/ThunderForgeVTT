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
