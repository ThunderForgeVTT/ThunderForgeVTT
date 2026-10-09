//! Spec 086 T043: SC-011 again, through the instruments that now exist.
//!
//! A canary-laden request goes through the real HTTP span, the real GraphQL
//! extension, a world event's span and the roll counter, into the installed
//! providers. The anonymous tier keeps only allow-listed attributes; the
//! operator tier keeps `world.id`.

use super::http::{HttpSpan, observe};
use super::install::{Installed, MemorySink, SamplerPlan, Sink, install, plan};
use super::settings::TelemetrySettings;
use super::tier::ANONYMOUS_SPAN_ATTRIBUTES;
use async_graphql::{EmptySubscription, Object, Schema};
use opentelemetry::metrics::MeterProvider as _;
use std::collections::HashMap;
use std::time::Instant;
use thunderforge_server::telemetry::graphql_extension::GraphQLTelemetry;
use thunderforge_server::telemetry::instruments::{Recorders, count_world_event};
use thunderforge_server::world_events::{EVENT_CODE_ROLL_MADE, record_span};
use tower_http::trace::MakeSpan as _;
use tracing_subscriber::Registry;
use tracing_subscriber::layer::SubscriberExt;

const INSTANCE: &str = "0b9d4c3e-5a8f-4f7e-9c2d-1e6b7a8c9d0f";
const OPERATOR: &str = "http://otel-collector.observability:4318";
const CANARY: &str = "zq-canary-91bd";
const WORLD: &str = "6f1c2b3a-4d5e-4f60-8a7b-9c0d1e2f3a4b";

struct Query;

#[Object]
impl Query {
    async fn hello(&self) -> bool {
        true
    }
}

struct Mutation;

#[Object]
impl Mutation {
    async fn send_chat_message(&self, text: String) -> bool {
        tracing::info!("chat: {text}");
        true
    }
}

fn installed(pairs: &[(&str, &str)], sink: &MemorySink) -> Installed {
    let env: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let mut p = plan(&TelemetrySettings::from_map(&env), INSTANCE, &env).expect("on");
    p.sampler = SamplerPlan::ParentBasedTraceIdRatio(1.0);
    install(Some(&p), Sink::Memory(sink.clone())).expect("installed")
}

fn play(t: &Installed) {
    let meter = t.meter_provider().expect("meter").meter("thunderforge");
    let r: &'static Recorders = Box::leak(Box::new(Recorders::new(&meter)));
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .extension(GraphQLTelemetry::with(r))
        .finish();
    let world: uuid::Uuid = WORLD.parse().unwrap();
    let subscriber = Registry::default().with(t.layers::<Registry>());
    tracing::subscriber::with_default(subscriber, || {
        let req = axum::http::Request::post(format!("/graphql?world={WORLD}"))
            .header("user-agent", CANARY)
            .body(())
            .unwrap();
        let http = HttpSpan.make_span(&req);
        let _h = http.enter();
        let query = format!(
            "mutation {CANARY_OP} {{ sendChatMessage(text: \"{CANARY} in {WORLD}\") }}",
            CANARY_OP = CANARY.replace('-', "_")
        );
        let res = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(schema.execute(query));
        assert!(res.errors.is_empty(), "{:?}", res.errors);

        let span = record_span(world, EVENT_CODE_ROLL_MADE);
        let _w = span.enter();
        let payload = serde_json::json!({ "visibility": "everyone", "label": CANARY });
        let v = count_world_event(r, EVENT_CODE_ROLL_MADE, Some(&payload), true);
        if let Some(v) = v {
            span.record("visibility", v);
        }
        observe(r, "/graphql".into(), "POST", 200, Instant::now());
    });
    t.force_flush();
}

fn attrs(span: &opentelemetry_sdk::trace::SpanData) -> Vec<(String, String)> {
    span.attributes
        .iter()
        .map(|kv| (kv.key.to_string(), kv.value.to_string()))
        .collect()
}

#[test]
fn the_real_spans_and_instruments_leak_nothing_anonymously() {
    let sink = MemorySink::default();
    let t = installed(&[], &sink);
    play(&t);
    let spans = sink.spans.get_finished_spans().expect("spans");
    let names: Vec<&str> = spans.iter().map(|s| s.name.as_ref()).collect();
    assert!(names.contains(&"HTTP POST unmatched"), "{names:?}");
    assert!(
        names.contains(&"graphql.mutation sendChatMessage"),
        "{names:?}"
    );
    assert!(names.contains(&"world_event.record"), "{names:?}");
    for span in &spans {
        for (k, v) in attrs(span) {
            assert!(
                ANONYMOUS_SPAN_ATTRIBUTES.contains(&k.as_str()),
                "{k} on {}",
                span.name
            );
            assert!(!v.contains(CANARY) && !v.contains(WORLD), "{k}={v}");
        }
    }
    let roll = spans
        .iter()
        .find(|s| s.name == "world_event.record")
        .unwrap();
    assert!(attrs(roll).contains(&("visibility".into(), "everyone".into())));

    let metrics = format!(
        "{:?}",
        sink.metrics.get_finished_metrics().expect("metrics")
    );
    for name in [
        "thunderforge.graphql.operation.duration",
        "thunderforge.http.server.duration",
        "thunderforge.world_events",
        "thunderforge.rolls",
    ] {
        assert!(metrics.contains(name), "{name}");
    }
    assert!(!metrics.contains(CANARY) && !metrics.contains(WORLD));
    assert!(!metrics.contains("world.id"));
}

#[test]
fn the_operator_tier_keeps_the_world_on_the_event_span() {
    let sink = MemorySink::default();
    let t = installed(&[("OTEL_EXPORTER_OTLP_ENDPOINT", OPERATOR)], &sink);
    play(&t);
    let spans = sink.spans.get_finished_spans().expect("spans");
    let roll = spans
        .iter()
        .find(|s| s.name == "world_event.record")
        .unwrap();
    assert!(
        attrs(roll).contains(&("world.id".into(), WORLD.into())),
        "{:?}",
        attrs(roll)
    );
    let gql = spans
        .iter()
        .find(|s| s.name.starts_with("graphql"))
        .unwrap();
    assert!(
        attrs(gql)
            .iter()
            .any(|(k, v)| k == "graphql.operation.name" && v.contains("canary")),
        "the operator sees the operation's name"
    );
}
