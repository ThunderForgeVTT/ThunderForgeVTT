//! Spec 080: range parsing, and the answers against a real RustFS
//! (`docker compose up -d rustfs`), mirroring the other storage tests.

use super::*;
use crate::storage::rustfs::write_object;
use axum::body::to_bytes;
use uuid::Uuid;

fn with(pairs: &[(HeaderName, &str)]) -> HeaderMap {
    let mut h = HeaderMap::new();
    for (name, value) in pairs {
        h.insert(name.clone(), HeaderValue::from_str(value).unwrap());
    }
    h
}

fn range(value: &str) -> HeaderMap {
    with(&[(header::RANGE, value)])
}

#[test]
fn one_range_in_each_form_is_taken() {
    assert_eq!(
        wanted_range(&range("bytes=0-99")).as_deref(),
        Some("bytes=0-99")
    );
    assert_eq!(
        wanted_range(&range("bytes=100-")).as_deref(),
        Some("bytes=100-")
    );
    assert_eq!(
        wanted_range(&range("bytes=-500")).as_deref(),
        Some("bytes=-500")
    );
    assert_eq!(
        wanted_range(&range("bytes=7-7")).as_deref(),
        Some("bytes=7-7")
    );
}

#[test]
fn several_ranges_or_nonsense_mean_the_whole_file() {
    for value in [
        "bytes=0-1,5-9",
        "bytes=9-1",
        "bytes=-0",
        "bytes=-",
        "bytes=a-b",
        "items=0-9",
        "bytes= 0-9x",
    ] {
        assert_eq!(wanted_range(&range(value)), None, "{value}");
    }
    assert_eq!(wanted_range(&HeaderMap::new()), None);
}

#[test]
fn if_range_trusts_only_a_strong_tag() {
    assert_eq!(if_range(&HeaderMap::new()), IfRange::Absent);
    assert_eq!(
        if_range(&with(&[(header::IF_RANGE, "\"abc\"")])),
        IfRange::Tag("\"abc\"".to_string())
    );
    assert_eq!(
        if_range(&with(&[(header::IF_RANGE, "W/\"abc\"")])),
        IfRange::Distrust
    );
    assert_eq!(
        if_range(&with(&[(
            header::IF_RANGE,
            "Wed, 21 Oct 2015 07:28:00 GMT"
        )])),
        IfRange::Distrust
    );
}

#[test]
fn a_416_names_the_size_and_carries_no_bytes() {
    let response = not_satisfiable(1234, &[(header::CACHE_CONTROL, "private")]);
    assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes */1234");
    assert_eq!(response.headers()[header::CACHE_CONTROL], "private");
}

/// 1000 bytes whose value is their offset mod 251, so any slice is
/// recognisable.
fn object_bytes() -> Vec<u8> {
    (0..1000u32).map(|i| (i % 251) as u8).collect()
}

async fn stored() -> (RustFsConfig, String) {
    let cfg = RustFsConfig::from_env();
    let key = format!("test/ranged/{}.bin", Uuid::now_v7());
    write_object(&cfg, &key, object_bytes(), "application/octet-stream")
        .await
        .expect("write the test object");
    (cfg, key)
}

const EXTRA: &[(HeaderName, &str)] = &[(header::CONTENT_TYPE, "image/webp")];

#[tokio::test]
async fn a_whole_request_says_parts_are_offered_and_names_the_version() {
    let (cfg, key) = stored().await;
    let response = serve(&cfg, &key, &HeaderMap::new(), EXTRA).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let h = response.headers().clone();
    assert_eq!(h[header::ACCEPT_RANGES], "bytes");
    assert_eq!(h[header::CONTENT_LENGTH], "1000");
    assert_eq!(h[header::CONTENT_TYPE], "image/webp");
    let tag = h[header::ETAG].to_str().unwrap();
    assert!(
        tag.starts_with('"') && !tag.starts_with("W/"),
        "strong tag: {tag}"
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body.as_ref(), object_bytes().as_slice());
}

#[tokio::test]
async fn a_range_gets_exactly_those_bytes() {
    let (cfg, key) = stored().await;
    let response = serve(&cfg, &key, &range("bytes=100-199"), EXTRA)
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        response.headers()[header::CONTENT_RANGE],
        "bytes 100-199/1000"
    );
    assert_eq!(response.headers()[header::CONTENT_LENGTH], "100");
    assert_eq!(response.headers()[header::ACCEPT_RANGES], "bytes");
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body.as_ref(), &object_bytes()[100..200]);
}

#[tokio::test]
async fn an_open_and_a_suffix_range_are_served() {
    let (cfg, key) = stored().await;
    let tail = serve(&cfg, &key, &range("bytes=990-"), EXTRA)
        .await
        .unwrap();
    assert_eq!(tail.headers()[header::CONTENT_RANGE], "bytes 990-999/1000");
    let suffix = serve(&cfg, &key, &range("bytes=-10"), EXTRA).await.unwrap();
    assert_eq!(
        suffix.headers()[header::CONTENT_RANGE],
        "bytes 990-999/1000"
    );
    let body = to_bytes(suffix.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body.as_ref(), &object_bytes()[990..]);
}

#[tokio::test]
async fn a_range_past_the_end_is_refused_with_the_real_size() {
    let (cfg, key) = stored().await;
    let response = serve(&cfg, &key, &range("bytes=5000-"), EXTRA)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes */1000");
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(body.is_empty());
}

#[tokio::test]
async fn a_matching_version_gets_the_part() {
    let (cfg, key) = stored().await;
    let whole = serve(&cfg, &key, &HeaderMap::new(), EXTRA).await.unwrap();
    let tag = whole.headers()[header::ETAG].to_str().unwrap().to_string();

    let headers = with(&[(header::RANGE, "bytes=0-9"), (header::IF_RANGE, &tag)]);
    let part = serve(&cfg, &key, &headers, EXTRA).await.unwrap();
    assert_eq!(part.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(part.headers()[header::ETAG].to_str().unwrap(), tag);
}

#[tokio::test]
async fn another_version_gets_the_whole_current_file() {
    let (cfg, key) = stored().await;
    let headers = with(&[
        (header::RANGE, "bytes=0-9"),
        (header::IF_RANGE, "\"not-this-version\""),
    ]);
    let response = serve(&cfg, &key, &headers, EXTRA).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get(header::CONTENT_RANGE).is_none());
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body.len(), 1000);
}

#[tokio::test]
async fn a_replaced_object_changes_its_version() {
    let (cfg, key) = stored().await;
    let before = serve(&cfg, &key, &HeaderMap::new(), EXTRA).await.unwrap();
    let before = before.headers()[header::ETAG].clone();
    write_object(&cfg, &key, vec![7u8; 1000], "application/octet-stream")
        .await
        .unwrap();
    let after = serve(&cfg, &key, &HeaderMap::new(), EXTRA).await.unwrap();
    assert_ne!(after.headers()[header::ETAG], before);
}
