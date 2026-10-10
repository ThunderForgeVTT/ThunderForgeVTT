//! The fixtures (T094, R26): each honest body decodes and re-encodes to the
//! same content, and `capture_server_fixtures` writes the server's own.

use crate::intake::{Batch, Format, Signal};
use crate::tests_support::fixture;

/// The honest bodies: the browser's OTLP/JSON from `packages/telemetry`
/// (`capture-browser.mts`), and the server's protobuf from its own exporter
/// (`capture_server_fixtures`). The browser sends no metrics.
pub const HONEST: &[(Signal, Format, &str)] = &[
    (Signal::Traces, Format::Json, "browser-traces.json"),
    (Signal::Logs, Format::Json, "browser-logs.json"),
    (Signal::Traces, Format::Protobuf, "server-traces.pb"),
    (Signal::Logs, Format::Protobuf, "server-logs.pb"),
    (Signal::Metrics, Format::Protobuf, "server-metrics.pb"),
];

#[test]
fn each_honest_body_decodes_and_reencodes_to_the_same_content() {
    for (signal, format, name) in HONEST {
        let bytes = fixture(name);
        let batch =
            Batch::decode(*signal, *format, &bytes).unwrap_or_else(|| panic!("{name} decodes"));
        let again = Batch::decode(*signal, Format::Protobuf, &batch.encode_protobuf()).unwrap();
        assert_eq!(batch, again, "{name} round-trips");
        if *format == Format::Protobuf {
            assert_eq!(
                batch.encode_protobuf(),
                bytes,
                "{name} re-encodes byte for byte"
            );
        }
        let records: usize = match &batch {
            Batch::Traces(r) => r
                .resource_spans
                .iter()
                .flat_map(|r| &r.scope_spans)
                .map(|s| s.spans.len())
                .sum(),
            Batch::Logs(r) => r
                .resource_logs
                .iter()
                .flat_map(|r| &r.scope_logs)
                .map(|s| s.log_records.len())
                .sum(),
            Batch::Metrics(r) => r
                .resource_metrics
                .iter()
                .flat_map(|r| &r.scope_metrics)
                .map(|s| s.metrics.len())
                .sum(),
        };
        assert!(records > 0, "{name} holds records");
    }
}

#[test]
fn the_browser_json_keeps_its_ids_and_typed_values() {
    let Some(Batch::Traces(t)) = Batch::decode(
        Signal::Traces,
        Format::Json,
        &fixture("browser-traces.json"),
    ) else {
        panic!("traces decode");
    };
    let spans = &t.resource_spans[0].scope_spans[0].spans;
    assert_eq!(spans[0].name, "engine.load");
    assert_eq!(spans[0].trace_id.len(), 16, "hex trace id decoded to bytes");
    assert_eq!(spans[1].parent_span_id, spans[0].span_id);
    assert!(spans[0].start_time_unix_nano > 0);
    let bytes_attr = spans[0]
        .attributes
        .iter()
        .find(|kv| kv.key == "bytes")
        .unwrap();
    use opentelemetry_proto::tonic::common::v1::any_value::Value;
    assert_eq!(
        bytes_attr.value.as_ref().unwrap().value,
        Some(Value::IntValue(18_400_000)),
        "intValue as a string decodes to an int"
    );
}

#[test]
fn each_hostile_body_is_one_drop_reason() {
    // The request-shaped reasons (rate limits, body size, overload, the
    // upstream) need no body of their own; `tests_drops` makes them.
    for (signal, name) in [
        (Signal::Traces, "hostile-unknown_service.json"),
        (Signal::Logs, "hostile-instance_id.json"),
        (Signal::Metrics, "hostile-metric_name.json"),
        (Signal::Logs, "hostile-attribute_too_large.json"),
    ] {
        assert!(
            Batch::decode(signal, Format::Json, &fixture(name)).is_some(),
            "{name} decodes"
        );
    }
    assert!(
        Batch::decode(
            Signal::Logs,
            Format::Json,
            &fixture("hostile-undecodable.txt")
        )
        .is_none()
    );
}

/// Writes `server-{traces,logs,metrics}.pb` from the server's own exporter
/// (`opentelemetry-otlp` over HTTP, as `apps/thunderforge` builds it) into a
/// socket that records each body. Run it when the server's exporter changes:
///
///   cargo test -p thunderforge-telemetry-gateway capture_server_fixtures -- --ignored
#[test]
#[ignore = "writes the fixtures; run by hand"]
fn capture_server_fixtures() {
    use opentelemetry::KeyValue;
    use opentelemetry::logs::{
        AnyValue, LogRecord as _, Logger as _, LoggerProvider as _, Severity,
    };
    use opentelemetry::metrics::MeterProvider as _;
    use opentelemetry::trace::{Span as _, Tracer as _, TracerProvider as _};
    use opentelemetry_otlp::{LogExporter, MetricExporter, SpanExporter, WithExportConfig as _};
    use opentelemetry_sdk::Resource;
    use std::io::{BufRead as _, BufReader, Read as _, Write as _};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let writer = std::thread::spawn(move || {
        let mut seen = std::collections::BTreeSet::new();
        while seen.len() < 3 {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
            let mut length = 0usize;
            loop {
                let mut h = String::new();
                reader.read_line(&mut h).unwrap();
                if h == "\r\n" || h.is_empty() {
                    break;
                }
                if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let signal = path.trim_start_matches("/v1/").to_owned();
            std::fs::write(dir.join(format!("server-{signal}.pb")), &body).unwrap();
            seen.insert(signal);
            let mut s = stream;
            s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
                .unwrap();
        }
    });

    // The anonymous tier's resource, built by hand (ANONYMOUS_RESOURCE_ATTRIBUTES).
    let resource = Resource::builder_empty()
        .with_attributes([
            KeyValue::new("service.name", "thunderforge"),
            KeyValue::new("service.version", "1.4.0"),
            KeyValue::new(
                "thunderforge.instance.id",
                "0192f1a4-7b3c-7d2e-9f10-3a4b5c6d7e8f",
            ),
            KeyValue::new("thunderforge.tier", "anonymous"),
            KeyValue::new("os.type", "linux"),
            KeyValue::new("host.arch", "x86_64"),
            KeyValue::new("deployment.environment", "production"),
        ])
        .build();

    let spans = SpanExporter::builder()
        .with_http()
        .with_endpoint(format!("{base}/v1/traces"))
        .build()
        .unwrap();
    let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_resource(resource.clone())
        .with_simple_exporter(spans)
        .build();
    let tracer = tracer_provider.tracer("thunderforge");
    let mut span = tracer.start("POST /graphql");
    span.set_attribute(KeyValue::new("http.request.method", "POST"));
    span.set_attribute(KeyValue::new("http.route", "/graphql"));
    span.set_attribute(KeyValue::new("http.response.status_code", 200));
    span.set_attribute(KeyValue::new("graphql.operation.type", "mutation"));
    span.set_attribute(KeyValue::new("graphql.root_field", "upsertToken"));
    span.add_event(
        "exception",
        vec![
            KeyValue::new("exception.type", "Forbidden"),
            KeyValue::new("exception.message", "not a member"),
        ],
    );
    span.end();
    tracer_provider.shutdown().unwrap();

    let logs = LogExporter::builder()
        .with_http()
        .with_endpoint(format!("{base}/v1/logs"))
        .build()
        .unwrap();
    let logger_provider = opentelemetry_sdk::logs::SdkLoggerProvider::builder()
        .with_resource(resource.clone())
        .with_simple_exporter(logs)
        .build();
    let logger = logger_provider.logger("thunderforge");
    let mut record = logger.create_log_record();
    record.set_event_name("server.error");
    record.set_severity_number(Severity::Error);
    record.set_severity_text("ERROR");
    record.set_body(AnyValue::from("server.error"));
    record.add_attribute("event.name", "server.error");
    record.add_attribute("error.type", "DatabaseError");
    record.add_attribute("error.message", "connection reset");
    logger.emit(record);
    logger_provider.shutdown().unwrap();

    let metrics = MetricExporter::builder()
        .with_http()
        .with_endpoint(format!("{base}/v1/metrics"))
        .build()
        .unwrap();
    let meter_provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(opentelemetry_sdk::metrics::PeriodicReader::builder(metrics).build())
        .build();
    let meter = meter_provider.meter("thunderforge");
    meter
        .u64_counter("thunderforge.rolls")
        .with_unit("{roll}")
        .build()
        .add(3, &[KeyValue::new("outcome", "ok")]);
    meter
        .f64_histogram("thunderforge.http.server.duration")
        .with_unit("s")
        .build()
        .record(
            0.042,
            &[
                KeyValue::new("route", "/graphql"),
                KeyValue::new("method", "POST"),
                KeyValue::new("status_class", "2xx"),
            ],
        );
    meter_provider.shutdown().unwrap();
    writer.join().unwrap();
}
