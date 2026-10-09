//! Spec 086 T042: the request span, its ids on Bunyan's lines, the remote
//! parent, and the duration's labels.

use super::*;
use crate::telemetry::bunyan::{Captured, formatter};
use axum::Router;
use axum::body::Body;
use axum::routing::get;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use tower::ServiceExt as _;
use tracing_bunyan_formatter::JsonStorageLayer;
use tracing_subscriber::Registry;
use tracing_subscriber::layer::SubscriberExt;

const PARENT_TRACE: &str = "4bf92f3577b34da6a3ce929d0e0e4736";

fn app() -> Router {
    Router::new()
        .route(
            "/worlds/{id}",
            get(|| async {
                tracing::info!(user = 1, "inside");
                "ok"
            }),
        )
        .layer(layer())
}

async fn call(app: Router, path: &str, traceparent: Option<&str>) -> u16 {
    let mut req = axum::http::Request::get(path);
    if let Some(tp) = traceparent {
        req = req.header("traceparent", tp);
    }
    let res = app.oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
    res.status().as_u16()
}

fn traced() -> (SdkTracerProvider, InMemorySpanExporter) {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    (provider, exporter)
}

#[tokio::test]
async fn a_bunyan_line_inside_a_request_holds_the_trace_id() {
    let (provider, exporter) = traced();
    let out = Captured::default();
    let sub = Registry::default()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("t")))
        .with(JsonStorageLayer)
        .with(formatter("t", out.clone()));
    let _g = tracing::subscriber::set_default(sub);
    assert_eq!(call(app(), "/worlds/abc", None).await, 200);

    let lines = out.lines();
    assert_eq!(lines.len(), 1, "no START or END line: {lines:?}");
    let trace_id = lines[0]["trace_id"].as_str().expect("trace_id");
    assert_eq!(trace_id.len(), 32);
    assert_eq!(lines[0]["span_id"].as_str().map(str::len), Some(16));
    for skipped in ["otel.name", "http.route", "http.request.method"] {
        assert!(lines[0].get(skipped).is_none(), "{skipped}: {lines:?}");
    }

    let spans = exporter.get_finished_spans().unwrap();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].name, "HTTP GET /worlds/{id}");
    assert_eq!(spans[0].span_context.trace_id().to_string(), trace_id);
    let attr = |k: &str| {
        spans[0]
            .attributes
            .iter()
            .find(|kv| kv.key.as_str() == k)
            .map(|kv| kv.value.to_string())
    };
    assert_eq!(attr("http.route").as_deref(), Some("/worlds/{id}"));
    assert_eq!(attr("http.request.method").as_deref(), Some("GET"));
    assert_eq!(attr("http.response.status_code").as_deref(), Some("200"));
}

#[tokio::test]
async fn an_incoming_traceparent_is_the_parent() {
    opentelemetry::global::set_text_map_propagator(
        opentelemetry_sdk::propagation::TraceContextPropagator::new(),
    );
    let (provider, exporter) = traced();
    let sub =
        Registry::default().with(tracing_opentelemetry::layer().with_tracer(provider.tracer("t")));
    let _g = tracing::subscriber::set_default(sub);
    let tp = format!("00-{PARENT_TRACE}-00f067aa0ba902b7-01");
    call(app(), "/worlds/abc", Some(&tp)).await;
    let spans = exporter.get_finished_spans().unwrap();
    assert_eq!(spans[0].span_context.trace_id().to_string(), PARENT_TRACE);
    assert_eq!(spans[0].parent_span_id.to_string(), "00f067aa0ba902b7");
}

#[tokio::test]
async fn without_opentelemetry_a_line_is_what_it_was() {
    let out = Captured::default();
    let sub = Registry::default()
        .with(JsonStorageLayer)
        .with(formatter("t", out.clone()));
    let _g = tracing::subscriber::set_default(sub);
    call(app(), "/worlds/abc", None).await;
    let lines = out.lines();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].get("trace_id").is_none());
    assert!(lines[0].get("span_id").is_none());
}

#[test]
fn the_labels_are_bounded() {
    assert_eq!(method(&Method::GET), "GET");
    assert_eq!(method(&Method::from_bytes(b"BREW").unwrap()), "other");
    for (code, class) in [
        (101, "1xx"),
        (204, "2xx"),
        (304, "3xx"),
        (429, "4xx"),
        (503, "5xx"),
    ] {
        assert_eq!(status_class(code), class);
    }
    let req = axum::http::Request::get("/worlds/6f1c2b3a")
        .body(())
        .unwrap();
    assert_eq!(route(&req), UNMATCHED, "a path is never a label");
}

#[test]
fn the_duration_carries_route_method_and_status_class() {
    let exporter = InMemoryMetricExporter::default();
    let provider = SdkMeterProvider::builder()
        .with_reader(PeriodicReader::builder(exporter.clone()).build())
        .build();
    use opentelemetry::metrics::MeterProvider as _;
    let r = Recorders::new(&provider.meter("t"));
    observe(&r, "/worlds/{id}".into(), "POST", 404, Instant::now());
    provider.force_flush().unwrap();
    let text = format!("{:?}", exporter.get_finished_metrics().unwrap());
    assert!(text.contains("thunderforge.http.server.duration"), "{text}");
    for v in ["/worlds/{id}", "POST", "4xx"] {
        assert!(text.contains(v), "{v}: {text}");
    }
    let _ = provider.shutdown();
}
