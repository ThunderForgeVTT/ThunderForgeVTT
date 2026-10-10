//! The gateway end to end, in process (T095, SC-014): preflight, every
//! honest fixture forwarded as protobuf, the labels per source, nothing of an
//! address or a user agent anywhere, stripping, and the collector's own
//! `partial_success` passed back. The drop reasons are in `tests_drops`.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use opentelemetry_proto::tonic::collector::logs::v1::{
    ExportLogsPartialSuccess, ExportLogsServiceResponse,
};
use opentelemetry_proto::tonic::common::v1::{KeyValue, any_value::Value};
use opentelemetry_proto::tonic::resource::v1::Resource;
use prost::Message as _;

use crate::fixture_tests::HONEST;
use crate::intake::{Batch, Format, Signal};
use crate::metrics::{ACCEPTED, ATTRIBUTES_STRIPPED};
use crate::tests_support::{CHROME_UA, Harness, Mode, body_bytes, fixture, request};

fn path(signal: Signal) -> String {
    format!("/v1/{}", signal.as_str())
}

fn content_type(format: Format) -> &'static str {
    format.content_type()
}

fn resources(signal: Signal, body: &[u8]) -> Vec<Resource> {
    match Batch::decode(signal, Format::Protobuf, body).expect("the fake received protobuf") {
        Batch::Traces(r) => r
            .resource_spans
            .into_iter()
            .filter_map(|r| r.resource)
            .collect(),
        Batch::Logs(r) => r
            .resource_logs
            .into_iter()
            .filter_map(|r| r.resource)
            .collect(),
        Batch::Metrics(r) => r
            .resource_metrics
            .into_iter()
            .filter_map(|r| r.resource)
            .collect(),
    }
}

fn attr<'a>(attrs: &'a [KeyValue], key: &str) -> Option<&'a str> {
    let values: Vec<&str> = attrs
        .iter()
        .filter(|kv| kv.key == key)
        .filter_map(|kv| match kv.value.as_ref()?.value.as_ref()? {
            Value::StringValue(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    assert!(values.len() <= 1, "{key} is set once, got {values:?}");
    values.first().copied()
}

#[tokio::test]
async fn the_preflight_answers_any_origin_without_credentials() {
    let h = Harness::new(&[]).await;
    for origin in ["https://game.example.org", "null"] {
        let req = Request::builder()
            .method(Method::OPTIONS)
            .uri("/v1/traces")
            .header("origin", origin)
            .header("access-control-request-method", "POST")
            .header("access-control-request-headers", "content-type")
            .body(Body::empty())
            .unwrap();
        let res = h.send(req).await;
        assert_eq!(res.status(), StatusCode::NO_CONTENT, "{origin}");
        let header = |name: &str| {
            res.headers()
                .get(name)
                .map(|v| v.to_str().unwrap().to_ascii_lowercase())
                .unwrap_or_default()
        };
        assert_eq!(header("access-control-allow-origin"), "*", "{origin}");
        assert!(
            header("access-control-allow-methods").contains("post"),
            "{origin}"
        );
        assert!(
            header("access-control-allow-headers").contains("content-type"),
            "{origin}"
        );
        assert_eq!(header("access-control-max-age"), "7200", "{origin}");
        assert!(
            res.headers()
                .get("access-control-allow-credentials")
                .is_none(),
            "{origin}"
        );
    }
    assert!(
        h.fake.received().is_empty(),
        "a preflight never reaches the collector"
    );
}

#[tokio::test]
async fn healthz_answers_without_the_upstream_and_anything_else_is_404() {
    let h = Harness::new(&[]).await;
    let res = h
        .send(Request::get("/healthz").body(Body::empty()).unwrap())
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let res = h
        .send(Request::post("/v1/profiles").body(Body::empty()).unwrap())
        .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let res = h
        .send(Request::get("/v1/traces").body(Body::empty()).unwrap())
        .await;
    assert_ne!(res.status(), StatusCode::OK);
    assert!(h.fake.received().is_empty());
}

#[tokio::test]
async fn each_honest_fixture_is_forwarded_as_protobuf_with_only_its_content_type() {
    let h = Harness::new(&[]).await;
    for (i, (signal, format, name)) in HONEST.iter().enumerate() {
        let origin: &[(&str, &str)] = if name.starts_with("browser") {
            &[("origin", "https://game.example.org")]
        } else {
            &[]
        };
        let res = h
            .send(request(
                &path(*signal),
                content_type(*format),
                fixture(name),
                origin,
            ))
            .await;
        assert_eq!(res.status(), StatusCode::OK, "{name}");
        assert_eq!(res.headers()["access-control-allow-origin"], "*", "{name}");
        assert_eq!(
            res.headers()["content-type"],
            format.content_type(),
            "{name}"
        );
        let received = h.fake.received();
        assert_eq!(received.len(), i + 1, "{name} was forwarded");
        let got = &received[i];
        assert_eq!(got.path, path(*signal), "{name}");
        let header = |k: &str| {
            got.headers
                .iter()
                .find(|(n, _)| n == k)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(
            header("content-type").as_deref(),
            Some(&b"application/x-protobuf"[..]),
            "{name}"
        );
        for forbidden in [
            "origin",
            "user-agent",
            "x-forwarded-for",
            "cf-ipcountry",
            "forwarded",
            "x-real-ip",
        ] {
            assert!(
                header(forbidden).is_none(),
                "{name}: {forbidden} is not forwarded"
            );
        }
        assert_eq!(resources(*signal, &got.body).len(), 1, "{name}");
    }
    assert_eq!(h.counter(ACCEPTED, ("signal", "traces")), 2);
    assert_eq!(h.counter(ACCEPTED, ("signal", "logs")), 2);
    assert_eq!(h.counter(ACCEPTED, ("signal", "metrics")), 1);
    assert_eq!(h.counter(ACCEPTED, ("source", "server")), 3);
    assert_eq!(h.counter(ACCEPTED, ("source", "self_hosted_browser")), 2);
}

#[tokio::test]
async fn a_metrics_body_in_json_is_forwarded_as_protobuf() {
    // The browser sends no metrics, so the JSON body is the server's own
    // fixture, re-encoded.
    let h = Harness::new(&[]).await;
    let Some(Batch::Metrics(m)) = Batch::decode(
        Signal::Metrics,
        Format::Protobuf,
        &fixture("server-metrics.pb"),
    ) else {
        panic!("metrics decode");
    };
    let json = serde_json::to_vec(&m).unwrap();
    let res = h
        .send(request("/v1/metrics", "application/json", json, &[]))
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let received = h.fake.received();
    assert_eq!(received.len(), 1);
    let Some(Batch::Metrics(forwarded)) =
        Batch::decode(Signal::Metrics, Format::Protobuf, &received[0].body)
    else {
        panic!("forwarded protobuf");
    };
    assert_eq!(
        forwarded.resource_metrics[0].scope_metrics,
        m.resource_metrics[0].scope_metrics
    );
}

#[tokio::test]
async fn every_label_is_set_for_each_source_with_and_without_a_country() {
    let h = Harness::new(&[]).await;
    let cases: &[(&str, &str, &[(&str, &str)], &str, Option<&str>)] = &[
        (
            "/v1/logs",
            "browser-logs.json",
            &[("origin", "https://thunderforge.dev")],
            "owner_site",
            Some("thunderforge.dev"),
        ),
        (
            "/v1/logs",
            "browser-logs.json",
            &[("origin", "https://Game.Example.org:8443")],
            "self_hosted_browser",
            Some("game.example.org"),
        ),
        (
            "/v1/logs",
            "browser-logs.json",
            &[("origin", "null")],
            "self_hosted_browser",
            Some("opaque"),
        ),
        ("/v1/logs", "server-logs.pb", &[], "server", None),
    ];
    for (path, name, headers, source, host) in cases {
        for country in [None, Some("DE")] {
            let mut all: Vec<(&str, &str)> = headers.to_vec();
            all.push((
                "user-agent",
                if *source == "server" {
                    "opentelemetry-otlp/0.33.0"
                } else {
                    CHROME_UA
                },
            ));
            if let Some(c) = country {
                all.push(("cf-ipcountry", c));
            }
            let format = if name.ends_with(".pb") {
                "application/x-protobuf"
            } else {
                "application/json"
            };
            let before = h.fake.received().len();
            let res = h.send(request(path, format, fixture(name), &all)).await;
            assert_eq!(res.status(), StatusCode::OK, "{name} {source}");
            let received = h.fake.received();
            assert_eq!(received.len(), before + 1, "{name} {source}");
            let r = &resources(Signal::Logs, &received[before].body)[0];
            let a = &r.attributes;
            assert_eq!(attr(a, "thunderforge.ingress"), Some("public"));
            assert_eq!(attr(a, "thunderforge.source"), Some(*source));
            assert_eq!(attr(a, "thunderforge.origin.host"), *host, "{source}");
            assert_eq!(
                attr(a, "thunderforge.instance.id"),
                Some("0192f1a4-7b3c-7d2e-9f10-3a4b5c6d7e8f")
            );
            assert_eq!(attr(a, "thunderforge.client.version"), Some("1.4.0"));
            let (family, major) = if *source == "server" {
                ("otel-rust", "0")
            } else {
                ("chrome", "131")
            };
            assert_eq!(
                attr(a, "thunderforge.user_agent.family"),
                Some(family),
                "{source}"
            );
            assert_eq!(
                attr(a, "thunderforge.user_agent.major"),
                Some(major),
                "{source}"
            );
            assert_eq!(attr(a, "thunderforge.country"), country, "{source}");
        }
    }
}

#[tokio::test]
async fn a_senders_own_label_is_overwritten_and_a_bad_country_is_left_out() {
    let h = Harness::new(&[]).await;
    let mut body: serde_json::Value =
        serde_json::from_slice(&fixture("browser-logs.json")).unwrap();
    let attrs = body["resourceLogs"][0]["resource"]["attributes"]
        .as_array_mut()
        .unwrap();
    attrs.push(
        serde_json::json!({"key": "thunderforge.source", "value": {"stringValue": "server"}}),
    );
    attrs.push(serde_json::json!({"key": "thunderforge.country", "value": {"stringValue": "FR"}}));
    for bad in ["XX", "T1", "de"] {
        let before = h.fake.received().len();
        let req = request(
            "/v1/logs",
            "application/json",
            serde_json::to_vec(&body).unwrap(),
            &[
                ("origin", "https://game.example.org"),
                ("cf-ipcountry", bad),
            ],
        );
        assert_eq!(h.send(req).await.status(), StatusCode::OK);
        let r = &resources(Signal::Logs, &h.fake.received()[before].body)[0];
        assert_eq!(
            attr(&r.attributes, "thunderforge.source"),
            Some("self_hosted_browser")
        );
        assert_eq!(attr(&r.attributes, "thunderforge.country"), None, "{bad}");
    }
    assert_eq!(
        h.counter(ATTRIBUTES_STRIPPED, ("signal", "logs")),
        0,
        "an overwritten label is not stripped"
    );
}

#[tokio::test]
async fn no_address_and_no_user_agent_reach_the_collector_the_logs_or_the_metrics() {
    let h = Harness::new(&[]).await;
    for (signal, format, name) in HONEST {
        let req = request(
            &path(*signal),
            content_type(*format),
            fixture(name),
            &[
                ("origin", "https://game.example.org"),
                ("x-forwarded-for", "203.0.113.77"),
                ("user-agent", CHROME_UA),
                ("cf-ipcountry", "DE"),
            ],
        );
        assert_eq!(h.send(req).await.status(), StatusCode::OK, "{name}");
    }
    let received = h.fake.received();
    assert_eq!(received.len(), HONEST.len());
    let logs = h.logs_text();
    assert!(logs.contains("answered"), "the trace layer logged: {logs}");
    let metrics = h.metrics_text();
    for needle in ["203.0.113.77", CHROME_UA, "AppleWebKit", "6778", "10.1.2.3"] {
        for got in &received {
            let bytes = got.all_bytes();
            assert!(
                !bytes.windows(needle.len()).any(|w| w == needle.as_bytes()),
                "{needle} reached the collector at {}",
                got.path
            );
        }
        assert!(!logs.contains(needle), "{needle} is in the logs");
        assert!(!metrics.contains(needle), "{needle} is in a metric");
    }
    for needle in ["game.example.org", "DE\"", "0192f1a4"] {
        assert!(!metrics.contains(needle), "{needle} is a metric attribute");
    }
}

#[tokio::test]
async fn an_unlisted_attribute_is_stripped_and_counted_not_dropped() {
    let h = Harness::new(&[]).await;
    let mut body: serde_json::Value =
        serde_json::from_slice(&fixture("browser-logs.json")).unwrap();
    body["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["attributes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"key": "user.email", "value": {"stringValue": "someone@example.com"}}));
    body["resourceLogs"][0]["resource"]["attributes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"key": "host.name", "value": {"stringValue": "laptop"}}));
    let res = h
        .send(request(
            "/v1/logs",
            "application/json",
            serde_json::to_vec(&body).unwrap(),
            &[("origin", "https://game.example.org")],
        ))
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let answer: serde_json::Value = serde_json::from_slice(&body_bytes(res).await).unwrap();
    assert!(
        answer
            .get("partialSuccess")
            .is_none_or(serde_json::Value::is_null),
        "{answer}"
    );
    let got = &h.fake.received()[0];
    for needle in ["someone@example.com", "laptop"] {
        assert!(
            !got.body
                .windows(needle.len())
                .any(|w| w == needle.as_bytes()),
            "{needle}"
        );
    }
    let Some(Batch::Logs(l)) = Batch::decode(Signal::Logs, Format::Protobuf, &got.body) else {
        panic!()
    };
    assert_eq!(
        l.resource_logs[0].scope_logs[0].log_records.len(),
        2,
        "both records kept"
    );
    assert_eq!(h.counter(ATTRIBUTES_STRIPPED, ("signal", "logs")), 2);
}

#[tokio::test]
async fn the_collectors_own_partial_success_is_passed_back() {
    let h = Harness::new(&[]).await;
    let theirs = ExportLogsServiceResponse {
        partial_success: Some(ExportLogsPartialSuccess {
            rejected_log_records: 2,
            error_message: "collector said no".into(),
        }),
    };
    h.fake.set(Mode::Answer(theirs.encode_to_vec()));
    let res = h
        .send(request(
            "/v1/logs",
            "application/x-protobuf",
            fixture("server-logs.pb"),
            &[],
        ))
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let ours = ExportLogsServiceResponse::decode(&body_bytes(res).await[..]).unwrap();
    let partial = ours.partial_success.expect("partial_success");
    assert_eq!(partial.rejected_log_records, 2);
    assert!(
        partial.error_message.contains("collector said no"),
        "{}",
        partial.error_message
    );
}
