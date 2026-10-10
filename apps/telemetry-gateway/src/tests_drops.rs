//! One test per drop reason (T095, SC-014): the status, what the fake
//! collector received, and `dropped{reason}` at one.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::http::StatusCode;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceResponse;
use prost::Message as _;
use thunderforge_telemetry_policy::DropReason;
use tokio::sync::Notify;
use tower::ServiceExt as _;

use crate::intake::{Batch, Format, Signal};
use crate::metrics::{DROPPED, UPSTREAM_DURATION};
use crate::tests_support::{Harness, Mode, PEER, body_bytes, fixture, request};

const BROWSER: &[(&str, &str)] = &[("origin", "https://game.example.org")];

fn browser_logs() -> axum::extract::Request {
    request(
        "/v1/logs",
        "application/json",
        fixture("browser-logs.json"),
        BROWSER,
    )
}

fn server_logs() -> axum::extract::Request {
    request(
        "/v1/logs",
        "application/x-protobuf",
        fixture("server-logs.pb"),
        &[],
    )
}

fn dropped(h: &Harness, reason: DropReason) -> u64 {
    h.counter(DROPPED, ("reason", reason.as_str()))
}

/// Only `reason` was counted, and once.
fn only(h: &Harness, reason: DropReason) {
    for r in DropReason::ALL {
        let want = u64::from(r == reason);
        assert_eq!(dropped(h, r), want, "dropped{{reason={}}}", r.as_str());
    }
}

/// Waits, without a clock, until the fake has received `n` requests.
async fn received(h: &Harness, n: usize) {
    for _ in 0..500 {
        if h.fake.received().len() >= n {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("the fake received {} of {n}", h.fake.received().len());
}

#[tokio::test]
async fn rate_limited_ip_answers_429_with_retry_after_until_the_clock_moves() {
    let h = Harness::new(&["--ip-rate", "0.5", "--ip-burst", "1"]).await;
    assert_eq!(h.send(server_logs()).await.status(), StatusCode::OK);
    let res = h.send(server_logs()).await;
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(res.headers()["retry-after"], "2");
    assert_eq!(res.headers()["access-control-allow-origin"], "*");
    assert_eq!(
        h.fake.received().len(),
        1,
        "the second never reached the collector"
    );
    only(&h, DropReason::RateLimitedIp);
    h.advance(Duration::from_millis(1500));
    let res = h.send(server_logs()).await;
    assert_eq!(res.headers()["retry-after"], "1", "rounded up");
    h.advance(Duration::from_millis(500));
    assert_eq!(h.send(server_logs()).await.status(), StatusCode::OK);
}

#[tokio::test]
async fn the_ip_is_the_forwarded_one_and_another_ip_has_its_own_bucket() {
    let h = Harness::new(&["--ip-rate", "0.1", "--ip-burst", "1"]).await;
    let from = |ip: &'static str| {
        request(
            "/v1/logs",
            "application/x-protobuf",
            fixture("server-logs.pb"),
            &[("x-forwarded-for", ip)],
        )
    };
    assert_eq!(h.send(from("203.0.113.77")).await.status(), StatusCode::OK);
    assert_eq!(h.send(from("203.0.113.78")).await.status(), StatusCode::OK);
    assert_eq!(
        h.send(from("203.0.113.77")).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(h.state.limits.ip_keys(), 2, "{PEER} never keyed a bucket");
}

#[tokio::test]
async fn rate_limited_instance_answers_429_when_every_resource_is_over() {
    let h = Harness::new(&["--instance-rate", "0.25", "--instance-burst", "1"]).await;
    assert_eq!(h.send(browser_logs()).await.status(), StatusCode::OK);
    let res = h.send(browser_logs()).await;
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(res.headers()["retry-after"], "4");
    assert_eq!(h.fake.received().len(), 1);
    only(&h, DropReason::RateLimitedInstance);
}

#[tokio::test]
async fn body_too_large_answers_413_and_counts_once() {
    let h = Harness::new(&[]).await;
    let big = vec![b' '; crate::router::BODY_LIMIT + 1];
    let mut req = request("/v1/logs", "application/json", big.clone(), BROWSER);
    req.headers_mut()
        .insert("content-length", big.len().to_string().parse().unwrap());
    assert_eq!(h.send(req).await.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert!(h.fake.received().is_empty());
    only(&h, DropReason::BodyTooLarge);
    // Streamed, with no length to check up front.
    let res = h
        .send(request("/v1/logs", "application/json", big, BROWSER))
        .await;
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(dropped(&h, DropReason::BodyTooLarge), 2);
}

#[tokio::test]
async fn undecodable_answers_400_for_a_bad_body_or_an_unknown_type() {
    let h = Harness::new(&[]).await;
    let res = h
        .send(request(
            "/v1/logs",
            "application/json",
            fixture("hostile-undecodable.txt"),
            BROWSER,
        ))
        .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(h.fake.received().is_empty());
    only(&h, DropReason::Undecodable);
    let res = h
        .send(request(
            "/v1/logs",
            "text/plain",
            fixture("browser-logs.json"),
            BROWSER,
        ))
        .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let res = h
        .send(request(
            "/v1/logs",
            "application/x-protobuf",
            vec![0xff; 8],
            &[],
        ))
        .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(dropped(&h, DropReason::Undecodable), 3);
}

#[tokio::test]
async fn unknown_service_is_dropped_with_partial_success() {
    let h = Harness::new(&[]).await;
    let res = h
        .send(request(
            "/v1/traces",
            "application/json",
            fixture("hostile-unknown_service.json"),
            BROWSER,
        ))
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let answer: serde_json::Value = serde_json::from_slice(&body_bytes(res).await).unwrap();
    assert_eq!(answer["partialSuccess"]["rejectedSpans"], 2, "{answer}");
    assert!(
        answer["partialSuccess"]["errorMessage"]
            .as_str()
            .unwrap()
            .contains("unknown_service")
    );
    assert!(h.fake.received().is_empty());
    only(&h, DropReason::UnknownService);
}

#[tokio::test]
async fn instance_id_is_dropped_when_malformed_or_missing() {
    let h = Harness::new(&[]).await;
    let res = h
        .send(request(
            "/v1/logs",
            "application/json",
            fixture("hostile-instance_id.json"),
            BROWSER,
        ))
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(h.fake.received().is_empty());
    only(&h, DropReason::InstanceId);

    // Absent: required of a self-hosted browser, not of the owner's landing.
    let mut body: serde_json::Value =
        serde_json::from_slice(&fixture("browser-logs.json")).unwrap();
    let attrs = body["resourceLogs"][0]["resource"]["attributes"]
        .as_array_mut()
        .unwrap();
    attrs.retain(|kv| kv["key"] != "thunderforge.instance.id");
    for kv in attrs.iter_mut() {
        if kv["key"] == "service.name" {
            kv["value"]["stringValue"] = "thunderforge-landing".into();
        }
    }
    let bytes = serde_json::to_vec(&body).unwrap();
    let res = h
        .send(request(
            "/v1/logs",
            "application/json",
            bytes.clone(),
            BROWSER,
        ))
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(h.fake.received().is_empty());
    assert_eq!(dropped(&h, DropReason::InstanceId), 2);
    let owner = &[("origin", "https://thunderforge.dev")];
    assert_eq!(
        h.send(request("/v1/logs", "application/json", bytes, owner))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        h.fake.received().len(),
        1,
        "the owner's landing needs no instance id"
    );
}

#[tokio::test]
async fn metric_name_drops_the_metric_with_partial_success() {
    let h = Harness::new(&[]).await;
    let res = h
        .send(request(
            "/v1/metrics",
            "application/x-protobuf",
            {
                let Some(b) = Batch::decode(
                    Signal::Metrics,
                    Format::Json,
                    &fixture("hostile-metric_name.json"),
                ) else {
                    panic!("decodes");
                };
                b.encode_protobuf()
            },
            &[],
        ))
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let answer = ExportMetricsServiceResponse::decode(&body_bytes(res).await[..]).unwrap();
    assert_eq!(answer.partial_success.unwrap().rejected_data_points, 1);
    assert!(h.fake.received().is_empty(), "nothing was left to forward");
    only(&h, DropReason::MetricName);
}

#[tokio::test]
async fn attribute_too_large_drops_the_record_and_forwards_the_rest() {
    let h = Harness::new(&[]).await;
    let res = h
        .send(request(
            "/v1/logs",
            "application/json",
            fixture("hostile-attribute_too_large.json"),
            BROWSER,
        ))
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let answer: serde_json::Value = serde_json::from_slice(&body_bytes(res).await).unwrap();
    assert_eq!(
        answer["partialSuccess"]["rejectedLogRecords"], 1,
        "{answer}"
    );
    let received = h.fake.received();
    assert_eq!(received.len(), 1);
    let Some(Batch::Logs(l)) = Batch::decode(Signal::Logs, Format::Protobuf, &received[0].body)
    else {
        panic!()
    };
    assert_eq!(l.resource_logs[0].scope_logs[0].log_records.len(), 1);
    assert!(
        !received[0]
            .body
            .windows(600)
            .any(|w| w.iter().all(|b| *b == b'x'))
    );
    only(&h, DropReason::AttributeTooLarge);
}

#[tokio::test]
async fn overloaded_answers_503_fast_when_the_collector_holds_every_slot() {
    let h = Harness::new(&["--upstream-in-flight", "1"]).await;
    let release = Arc::new(Notify::new());
    h.fake.set(Mode::Hold(release.clone()));
    let mut held = server_logs();
    held.extensions_mut().insert(axum::extract::ConnectInfo(
        PEER.parse::<std::net::SocketAddr>().unwrap(),
    ));
    let first = tokio::spawn(h.app.clone().oneshot(held));
    received(&h, 1).await;
    let started = Instant::now();
    let res = h.send(server_logs()).await;
    let took = started.elapsed();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(took < Duration::from_millis(50), "took {took:?}");
    assert_eq!(h.fake.received().len(), 1);
    only(&h, DropReason::Overloaded);
    release.notify_one();
    assert_eq!(first.await.unwrap().unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn overloaded_answers_503_when_the_concurrency_limit_is_full() {
    let h = Harness::new(&["--concurrency", "1"]).await;
    let release = Arc::new(Notify::new());
    h.fake.set(Mode::Hold(release.clone()));
    let first = tokio::spawn(h.app.clone().oneshot(server_logs()));
    received(&h, 1).await;
    let res = h
        .send(request(
            "/v1/traces",
            "application/json",
            fixture("browser-traces.json"),
            BROWSER,
        ))
        .await;
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(res.headers()["access-control-allow-origin"], "*");
    only(&h, DropReason::Overloaded);
    release.notify_one();
    assert_eq!(first.await.unwrap().unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn upstream_error_answers_503_after_trying() {
    let h = Harness::new(&[]).await;
    h.fake.set(Mode::Status(500));
    let res = h
        .send(request(
            "/v1/traces",
            "application/json",
            fixture("browser-traces.json"),
            BROWSER,
        ))
        .await;
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(h.fake.received().len(), 1, "attempted");
    only(&h, DropReason::UpstreamError);
    assert!(h.metrics_text().contains(UPSTREAM_DURATION));
}
