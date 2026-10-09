//! Spec 086 T020 and T021: the three states (SC-001) and the leak test
//! (SC-011), through in-memory exporters.

use super::install::{Installed, MemorySink, SamplerPlan, Sink, install, plan};
use super::settings::TelemetrySettings;
use super::tier::{
    ANONYMOUS_RESOURCE_ATTRIBUTES, ANONYMOUS_SPAN_ATTRIBUTES, PROJECT_TELEMETRY_ENDPOINT, Tier,
};
use opentelemetry::metrics::MeterProvider as _;
use std::collections::HashMap;
use thunderforge_telemetry_policy::{SERVER_ERROR_ATTRIBUTES, SERVER_SPAN_EVENT_ATTRIBUTES};
use tracing_subscriber::Registry;
use tracing_subscriber::layer::SubscriberExt;

const INSTANCE: &str = "0b9d4c3e-5a8f-4f7e-9c2d-1e6b7a8c9d0f";
const OPERATOR: &str = "http://otel-collector.observability:4318";

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn plan_for(pairs: &[(&str, &str)]) -> Option<super::install::Plan> {
    let env = env(pairs);
    plan(&TelemetrySettings::from_map(&env), INSTANCE, &env)
}

fn resource_keys(r: &opentelemetry_sdk::Resource) -> Vec<String> {
    let mut keys: Vec<String> = r.iter().map(|(k, _)| k.to_string()).collect();
    keys.sort();
    keys
}

#[test]
fn telemetry_false_installs_nothing() {
    for v in ["false", "0", "off"] {
        let p = plan_for(&[("TELEMETRY", v)]);
        assert!(p.is_none(), "{v}");
        assert!(install(p.as_ref(), Sink::Memory(MemorySink::default())).is_none());
    }
}

#[test]
fn the_default_is_anonymous_to_the_project() {
    let p = plan_for(&[]).expect("on by default");
    assert_eq!(p.tier, Tier::Anonymous);
    for (got, path) in [
        (&p.traces, "/v1/traces"),
        (&p.metrics, "/v1/metrics"),
        (&p.logs, "/v1/logs"),
    ] {
        assert_eq!(
            got.as_deref(),
            Some(format!("{PROJECT_TELEMETRY_ENDPOINT}{path}").as_str())
        );
    }
    assert!(!p.log_bridge, "no log bridge on the anonymous tier");
    assert_eq!(p.sampler, SamplerPlan::ParentBasedTraceIdRatio(0.1));
    let mut want: Vec<String> = ANONYMOUS_RESOURCE_ATTRIBUTES
        .iter()
        .map(|s| s.to_string())
        .collect();
    want.sort();
    assert_eq!(resource_keys(&p.resource), want);
    let get = |k: &'static str| {
        p.resource
            .get(&opentelemetry::Key::from_static_str(k))
            .map(|v| v.to_string())
    };
    assert_eq!(
        get("deployment.environment").as_deref(),
        Some("self-hosted")
    );
    assert_eq!(get("thunderforge.instance.id").as_deref(), Some(INSTANCE));
    assert_eq!(get("thunderforge.tier").as_deref(), Some("anonymous"));
}

#[test]
fn a_general_endpoint_is_operator_with_the_bridge_and_detectors() {
    let p = plan_for(&[("OTEL_EXPORTER_OTLP_ENDPOINT", OPERATOR)]).expect("on");
    assert_eq!(p.tier, Tier::Operator);
    assert_eq!(p.traces.as_deref(), Some(&*format!("{OPERATOR}/v1/traces")));
    assert_eq!(
        p.metrics.as_deref(),
        Some(&*format!("{OPERATOR}/v1/metrics"))
    );
    assert_eq!(p.logs.as_deref(), Some(&*format!("{OPERATOR}/v1/logs")));
    assert!(p.log_bridge);
    assert_eq!(p.sampler, SamplerPlan::FromEnv);
    let keys = resource_keys(&p.resource);
    assert!(keys.contains(&"telemetry.sdk.name".to_string()), "{keys:?}");
    assert!(keys.contains(&"thunderforge.instance.id".to_string()));
}

#[test]
fn an_operator_signal_left_unset_goes_nowhere() {
    let traces = format!("{OPERATOR}/v1/traces");
    let p = plan_for(&[("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT", &traces)]).expect("on");
    assert_eq!(p.tier, Tier::Operator);
    assert_eq!(p.traces.as_deref(), Some(traces.as_str()));
    assert_eq!(
        p.metrics, None,
        "never the project's endpoint with operator data"
    );
    assert_eq!(p.logs, None);
}

#[test]
fn a_mixed_set_never_sends_operator_data_to_the_project() {
    let traces = format!("{OPERATOR}/v1/traces");
    let p = plan_for(&[
        ("OTEL_EXPORTER_OTLP_ENDPOINT", PROJECT_TELEMETRY_ENDPOINT),
        ("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT", &traces),
    ])
    .expect("on");
    assert_eq!(p.tier, Tier::Operator);
    assert_eq!(p.traces.as_deref(), Some(traces.as_str()));
    assert_eq!(p.metrics, None);
    assert_eq!(p.logs, None);
}

#[test]
fn sdk_disabled_installs_nothing_and_leaves_the_served_config() {
    let on = TelemetrySettings::from_map(&env(&[]));
    let disabled_env = env(&[("OTEL_SDK_DISABLED", "true")]);
    let disabled = TelemetrySettings::from_map(&disabled_env);
    assert!(plan(&disabled, INSTANCE, &disabled_env).is_none());
    assert_eq!(
        disabled.browser(INSTANCE).served_json(),
        on.browser(INSTANCE).served_json()
    );
}

// ---- SC-011: the leak test -------------------------------------------------

const CANARY: &str = "zq-canary-7f3a";
const EMAIL: &str = "canary-7f3a@example.org";
const WORLD: &str = "6f1c2b3a-4d5e-4f60-8a7b-9c0d1e2f3a4b";
const TOKEN: &str = "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d";

fn installed(pairs: &[(&str, &str)], sink: &MemorySink) -> Installed {
    let mut p = plan_for(pairs).expect("on");
    // Every span sampled, so the test sees what a sampled one would carry.
    p.sampler = SamplerPlan::ParentBasedTraceIdRatio(1.0);
    install(Some(&p), Sink::Memory(sink.clone())).expect("installed")
}

/// What the server does with a canary-laden request, through `tracing`.
fn play(installed: &Installed) {
    let subscriber = Registry::default().with(installed.layers::<Registry>());
    tracing::subscriber::with_default(subscriber, || {
        let http = tracing::info_span!(
            "HTTP request",
            http.route = "/worlds/{id}",
            http.request.method = "POST",
            http.response.status_code = 200,
            url.path = %format!("/worlds/{WORLD}"),
            user_agent.original = "Mozilla/5.0 canary",
        );
        let _h = http.enter();
        let gql = tracing::info_span!(
            "graphql.mutation sendChatMessage",
            graphql.operation.type = "mutation",
            graphql.root_field = "sendChatMessage",
            graphql.operation.name = %CANARY,
            world.id = %WORLD,
            world_id = %WORLD,
            outcome = "ok",
        );
        let _g = gql.enter();
        tracing::info!(token_id = %TOKEN, actor = %CANARY, "chat line: {CANARY} says hi to {EMAIL}");
        // The spec's ERROR holds the email (and here an id). Free text such
        // as a world's name is never put in an error message by the server.
        tracing::error!(
            world_id = %WORLD,
            actor = %CANARY,
            "could not deliver the invite to {EMAIL} for world {WORLD}"
        );
    });
    installed.force_flush();
}

fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|h| h.trim().to_string())
        .unwrap_or_default()
}

fn assert_clean(what: &str, text: &str) {
    for secret in [EMAIL, CANARY, WORLD, TOKEN] {
        assert!(!text.contains(secret), "{what} leaks {secret}: {text}");
    }
    let host = hostname();
    if host.len() > 3 {
        assert!(!text.contains(&host), "{what} leaks the hostname: {text}");
    }
}

#[test]
fn the_anonymous_tier_leaks_nothing() {
    let sink = MemorySink::default();
    let t = installed(&[], &sink);
    play(&t);

    let spans = sink.spans.get_finished_spans().expect("spans");
    assert_eq!(spans.len(), 2, "both spans exported");
    for span in &spans {
        assert_clean("span name", &span.name);
        for kv in &span.attributes {
            assert!(
                ANONYMOUS_SPAN_ATTRIBUTES.contains(&kv.key.as_str()),
                "span attribute {} is not on the list",
                kv.key
            );
            assert_clean("span attribute", &kv.value.to_string());
        }
        assert!(span.links.links.is_empty());
        for event in &span.events.events {
            assert_eq!(event.name, "exception");
            for kv in &event.attributes {
                assert!(SERVER_SPAN_EVENT_ATTRIBUTES.contains(&kv.key.as_str()));
                assert_clean("span event", &kv.value.to_string());
            }
        }
        assert_clean("span status", &format!("{:?}", span.status));
    }
    let gql = spans
        .iter()
        .find(|s| s.name.starts_with("graphql"))
        .expect("graphql span");
    let keys: Vec<&str> = gql.attributes.iter().map(|kv| kv.key.as_str()).collect();
    assert!(keys.contains(&"graphql.root_field"), "{keys:?}");

    let logs = sink.logs.get_emitted_logs().expect("logs");
    assert_eq!(logs.len(), 1, "only the server.error record leaves");
    let log = &logs[0];
    assert_eq!(log.record.event_name(), Some("server.error"));
    assert_clean("log body", &format!("{:?}", log.record.body()));
    for (k, v) in log.record.attributes_iter() {
        assert!(
            SERVER_ERROR_ATTRIBUTES.contains(&k.as_str()),
            "log attribute {k}"
        );
        assert_clean("log attribute", &format!("{v:?}"));
    }
    let message = log
        .record
        .attributes_iter()
        .find(|(k, _)| k.as_str() == "error.message")
        .map(|(_, v)| format!("{v:?}"))
        .unwrap_or_default();
    assert!(message.contains("[redacted: email]"), "{message}");

    for (k, v) in log.resource.iter() {
        assert!(
            ANONYMOUS_RESOURCE_ATTRIBUTES.contains(&k.as_str()),
            "resource {k}"
        );
        assert_clean("resource", &v.to_string());
    }
}

#[test]
fn the_anonymous_tier_sends_only_the_contracts_instruments_and_labels() {
    let sink = MemorySink::default();
    let t = installed(&[], &sink);
    let meter = t.meter_provider().expect("meter").meter("thunderforge");
    let ok = meter.u64_counter("thunderforge.world_events").build();
    ok.add(
        1,
        &[
            opentelemetry::KeyValue::new("event", "roll_made"),
            opentelemetry::KeyValue::new("world.id", WORLD),
        ],
    );
    meter
        .u64_counter("someone.elses.counter")
        .build()
        .add(1, &[]);
    t.force_flush();
    let exported = format!(
        "{:?}",
        sink.metrics.get_finished_metrics().expect("metrics")
    );
    assert!(exported.contains("thunderforge.world_events"), "{exported}");
    assert!(!exported.contains("someone.elses.counter"), "{exported}");
    assert_clean("metrics", &exported);
}

#[test]
fn the_operator_tier_keeps_the_world() {
    let sink = MemorySink::default();
    let t = installed(&[("OTEL_EXPORTER_OTLP_ENDPOINT", OPERATOR)], &sink);
    play(&t);
    let spans = sink.spans.get_finished_spans().expect("spans");
    let gql = spans
        .iter()
        .find(|s| s.name.starts_with("graphql"))
        .expect("graphql span");
    assert!(
        gql.attributes
            .iter()
            .any(|kv| matches!(kv.key.as_str(), "world.id" | "world_id")
                && kv.value.to_string() == WORLD),
        "{:?}",
        gql.attributes
    );
    let logs = sink.logs.get_emitted_logs().expect("logs");
    assert!(logs.len() >= 2, "the log bridge carries every event");
}
