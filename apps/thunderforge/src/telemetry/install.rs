//! The providers, per tier (spec 086 FR-001 – FR-005, R1).
//!
//! [`plan`] decides, as plain data, what is exported where; [`install`] builds
//! the providers from a plan. The batch processors and the metric reader run
//! on their own threads with bounded queues, so nothing here waits on an
//! exporter, and a full queue drops rather than blocks (FR-003).

use super::anonymous::{AllowListSpanProcessor, ServerErrorLayer};
use super::settings::TelemetrySettings;
use super::tier::{
    ANONYMOUS_RESOURCE_ATTRIBUTES, INSTRUMENTS, PROJECT_TELEMETRY_ENDPOINT, Tier, tier_for,
};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::{Key, KeyValue};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::{BatchLogProcessor, LogExporter, SdkLoggerProvider};
use opentelemetry_sdk::metrics::{
    Aggregation, Instrument, PeriodicReader, SdkMeterProvider, Stream, exporter::PushMetricExporter,
};
use opentelemetry_sdk::trace::{BatchSpanProcessor, Sampler, SdkTracerProvider, SpanExporter};
use std::collections::HashMap;
use std::time::Duration;
use thunderforge_telemetry_policy::{InstrumentKind, SERVER_METRIC_ATTRIBUTES};
use tracing::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::registry::LookupSpan;

/// The anonymous tier's head sampler: `parentbased_traceidratio`, 0.1.
pub const ANONYMOUS_SAMPLE_RATIO: f64 = 0.1;
/// How often the metric reader collects.
pub const METRIC_INTERVAL: Duration = Duration::from_secs(60);
const EXPORT_TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

const SECONDS_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];
const POOL_WAIT_BUCKETS: &[f64] = &[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 30.0];
const POOL_WAIT: &str = "thunderforge.db.pool.checkout_wait";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    Traces,
    Metrics,
    Logs,
}

impl Signal {
    fn path(self) -> &'static str {
        match self {
            Signal::Traces => "/v1/traces",
            Signal::Metrics => "/v1/metrics",
            Signal::Logs => "/v1/logs",
        }
    }

    fn env(self) -> &'static str {
        match self {
            Signal::Traces => "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
            Signal::Metrics => "OTEL_EXPORTER_OTLP_METRICS_ENDPOINT",
            Signal::Logs => "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SamplerPlan {
    /// `parentbased_traceidratio` with this ratio.
    ParentBasedTraceIdRatio(f64),
    /// `OTEL_TRACES_SAMPLER`, read by the SDK.
    FromEnv,
}

/// What [`install`] builds. A signal with no endpoint is not exported.
#[derive(Clone, Debug)]
pub struct Plan {
    pub tier: Tier,
    pub traces: Option<String>,
    pub metrics: Option<String>,
    pub logs: Option<String>,
    /// Operator tier: every `tracing` event becomes a log record. Anonymous:
    /// only `server.error`.
    pub log_bridge: bool,
    pub sampler: SamplerPlan,
    pub resource: Resource,
}

/// `None` when nothing is to be exported: `TELEMETRY=false` or
/// `OTEL_SDK_DISABLED=true`.
pub fn plan(
    settings: &TelemetrySettings,
    instance_id: &str,
    env: &HashMap<String, String>,
) -> Option<Plan> {
    if !settings.exporting() {
        return None;
    }
    let tier = settings.server_tier;
    let get = |k: &str| {
        env.get(k)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let general = get("OTEL_EXPORTER_OTLP_ENDPOINT");
    let endpoint = |signal: Signal| {
        if let Some(url) = get(signal.env()) {
            return Some(url);
        }
        let base = match &general {
            Some(general) if tier == Tier::Operator && tier_for(general) == Tier::Anonymous => {
                return None;
            }
            Some(general) => general.as_str(),
            // Operator-tier data never goes to the project, even for a
            // signal the operator left unset.
            None if tier == Tier::Operator => return None,
            None => PROJECT_TELEMETRY_ENDPOINT,
        };
        Some(format!("{}{}", base.trim_end_matches('/'), signal.path()))
    };
    let (sampler, resource) = match tier {
        Tier::Anonymous => (
            SamplerPlan::ParentBasedTraceIdRatio(ANONYMOUS_SAMPLE_RATIO),
            anonymous_resource(instance_id),
        ),
        Tier::Operator => (
            SamplerPlan::FromEnv,
            operator_resource(instance_id, get("OTEL_SERVICE_NAME").is_none()),
        ),
    };
    Some(Plan {
        tier,
        traces: endpoint(Signal::Traces),
        metrics: endpoint(Signal::Metrics),
        logs: endpoint(Signal::Logs),
        log_bridge: tier == Tier::Operator,
        sampler,
        resource,
    })
}

/// Built by hand: exactly `ANONYMOUS_RESOURCE_ATTRIBUTES`, no detectors.
pub fn anonymous_resource(instance_id: &str) -> Resource {
    let attrs = [
        KeyValue::new("service.name", "thunderforge"),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        KeyValue::new("thunderforge.instance.id", instance_id.to_string()),
        KeyValue::new("thunderforge.tier", Tier::Anonymous.as_str()),
        KeyValue::new("os.type", std::env::consts::OS),
        KeyValue::new("host.arch", std::env::consts::ARCH),
        KeyValue::new("deployment.environment", "self-hosted"),
    ];
    debug_assert_eq!(attrs.len(), ANONYMOUS_RESOURCE_ATTRIBUTES.len());
    Resource::builder_empty().with_attributes(attrs).build()
}

/// The SDK's detectors and `OTEL_RESOURCE_ATTRIBUTES`, plus ours.
pub fn operator_resource(instance_id: &str, default_service_name: bool) -> Resource {
    let mut builder = Resource::builder().with_attributes([
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        KeyValue::new("thunderforge.instance.id", instance_id.to_string()),
        KeyValue::new("thunderforge.tier", Tier::Operator.as_str()),
    ]);
    if default_service_name {
        builder = builder.with_service_name("thunderforge");
    }
    builder.build()
}

fn buckets(name: &str) -> Option<&'static [f64]> {
    if name == POOL_WAIT {
        return Some(POOL_WAIT_BUCKETS);
    }
    INSTRUMENTS
        .iter()
        .any(|(n, kind, unit)| *n == name && *kind == InstrumentKind::Histogram && *unit == "s")
        .then_some(SECONDS_BUCKETS)
}

fn histogram(bounds: &'static [f64]) -> Aggregation {
    Aggregation::ExplicitBucketHistogram {
        boundaries: bounds.to_vec(),
        record_min_max: true,
    }
}

/// Anonymous: only the contract's instruments, only their labels.
fn anonymous_view(i: &Instrument) -> Option<Stream> {
    if !INSTRUMENTS.iter().any(|(n, _, _)| *n == i.name()) {
        return Stream::builder()
            .with_aggregation(Aggregation::Drop)
            .build()
            .ok();
    }
    let mut stream = Stream::builder().with_allowed_attribute_keys(
        SERVER_METRIC_ATTRIBUTES
            .iter()
            .map(|k| Key::from_static_str(k)),
    );
    if let Some(bounds) = buckets(i.name()) {
        stream = stream.with_aggregation(histogram(bounds));
    }
    stream.build().ok()
}

/// Operator: everything, with the contract's buckets.
fn operator_view(i: &Instrument) -> Option<Stream> {
    let bounds = buckets(i.name())?;
    Stream::builder()
        .with_aggregation(histogram(bounds))
        .build()
        .ok()
}

/// Where the exporters send. Tests swap in memory.
pub enum Sink {
    Otlp,
    #[cfg(test)]
    Memory(MemorySink),
}

#[cfg(test)]
#[derive(Clone, Debug, Default)]
pub struct MemorySink {
    pub spans: opentelemetry_sdk::trace::InMemorySpanExporter,
    pub logs: opentelemetry_sdk::logs::InMemoryLogExporter,
    pub metrics: opentelemetry_sdk::metrics::InMemoryMetricExporter,
}

/// The installed providers. Dropping it flushes and shuts them down.
#[derive(Debug)]
pub struct Installed {
    pub log_bridge: bool,
    tracer: Option<SdkTracerProvider>,
    meter: Option<SdkMeterProvider>,
    logger: Option<SdkLoggerProvider>,
}

fn traces_with<E: SpanExporter + 'static>(plan: &Plan, exporter: E) -> SdkTracerProvider {
    let batch = BatchSpanProcessor::builder(exporter).build();
    let mut builder = SdkTracerProvider::builder().with_resource(plan.resource.clone());
    if let SamplerPlan::ParentBasedTraceIdRatio(ratio) = plan.sampler {
        builder = builder.with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
            ratio,
        ))));
    }
    match plan.tier {
        Tier::Anonymous => builder.with_span_processor(AllowListSpanProcessor::new(batch)),
        Tier::Operator => builder.with_span_processor(batch),
    }
    .build()
}

fn logs_with<E: LogExporter + 'static>(plan: &Plan, exporter: E) -> SdkLoggerProvider {
    SdkLoggerProvider::builder()
        .with_resource(plan.resource.clone())
        .with_log_processor(BatchLogProcessor::builder(exporter).build())
        .build()
}

fn metrics_with<E: PushMetricExporter>(plan: &Plan, exporter: E) -> SdkMeterProvider {
    let reader = PeriodicReader::builder(exporter)
        .with_interval(METRIC_INTERVAL)
        .build();
    let builder = SdkMeterProvider::builder()
        .with_resource(plan.resource.clone())
        .with_reader(reader);
    match plan.tier {
        Tier::Anonymous => builder.with_view(anonymous_view),
        Tier::Operator => builder.with_view(operator_view),
    }
    .build()
}

fn warn(signal: &str, err: impl std::fmt::Display) {
    eprintln!(
        "[Server] ⚠️  Telemetry: no {signal} exporter ({err}); the server runs on without it."
    );
}

fn otlp(plan: &Plan) -> Installed {
    use opentelemetry_otlp::{LogExporter, MetricExporter, SpanExporter, WithExportConfig};
    let tracer = plan.traces.as_ref().and_then(|url| {
        SpanExporter::builder()
            .with_http()
            .with_endpoint(url)
            .with_timeout(EXPORT_TIMEOUT)
            .build()
            .map_err(|e| warn("trace", e))
            .ok()
            .map(|e| traces_with(plan, e))
    });
    let meter = plan.metrics.as_ref().and_then(|url| {
        MetricExporter::builder()
            .with_http()
            .with_endpoint(url)
            .with_timeout(EXPORT_TIMEOUT)
            .build()
            .map_err(|e| warn("metric", e))
            .ok()
            .map(|e| metrics_with(plan, e))
    });
    let logger = plan.logs.as_ref().and_then(|url| {
        LogExporter::builder()
            .with_http()
            .with_endpoint(url)
            .with_timeout(EXPORT_TIMEOUT)
            .build()
            .map_err(|e| warn("log", e))
            .ok()
            .map(|e| logs_with(plan, e))
    });
    Installed {
        log_bridge: plan.log_bridge,
        tracer,
        meter,
        logger,
    }
}

/// Builds the providers. `None` in, `None` out: off builds no exporter.
pub fn install(plan: Option<&Plan>, sink: Sink) -> Option<Installed> {
    let plan = plan?;
    match sink {
        Sink::Otlp => Some(otlp(plan)),
        #[cfg(test)]
        Sink::Memory(m) => Some(Installed {
            log_bridge: plan.log_bridge,
            tracer: plan.traces.as_ref().map(|_| traces_with(plan, m.spans)),
            meter: plan.metrics.as_ref().map(|_| metrics_with(plan, m.metrics)),
            logger: plan.logs.as_ref().map(|_| logs_with(plan, m.logs)),
        }),
    }
}

impl Installed {
    /// The `tracing` layers: spans to the tracer, and either the log bridge
    /// (operator) or the `server.error` layer (anonymous).
    pub fn layers<S>(&self) -> Vec<Box<dyn Layer<S> + Send + Sync + 'static>>
    where
        S: Subscriber + for<'a> LookupSpan<'a> + Send + Sync,
    {
        let mut layers: Vec<Box<dyn Layer<S> + Send + Sync + 'static>> = Vec::new();
        if let Some(tp) = &self.tracer {
            layers.push(Box::new(
                tracing_opentelemetry::layer().with_tracer(tp.tracer("thunderforge")),
            ));
        }
        if let Some(lp) = &self.logger {
            if self.log_bridge {
                layers.push(Box::new(
                    opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(lp),
                ));
            } else {
                use opentelemetry::logs::LoggerProvider as _;
                layers.push(Box::new(ServerErrorLayer::new(lp.logger("thunderforge"))));
            }
        }
        layers
    }

    /// The global meter, tracer and W3C propagator, for instruments that
    /// reach them through `opentelemetry::global`.
    pub fn set_global(&self) {
        if let Some(tp) = &self.tracer {
            opentelemetry::global::set_tracer_provider(tp.clone());
        }
        if let Some(mp) = &self.meter {
            opentelemetry::global::set_meter_provider(mp.clone());
        }
        opentelemetry::global::set_text_map_propagator(
            opentelemetry_sdk::propagation::TraceContextPropagator::new(),
        );
    }

    #[cfg(test)]
    pub fn force_flush(&self) {
        if let Some(tp) = &self.tracer {
            let _ = tp.force_flush();
        }
        if let Some(mp) = &self.meter {
            let _ = mp.force_flush();
        }
        if let Some(lp) = &self.logger {
            let _ = lp.force_flush();
        }
    }

    #[cfg(test)]
    pub fn meter_provider(&self) -> Option<&SdkMeterProvider> {
        self.meter.as_ref()
    }
}

impl Drop for Installed {
    fn drop(&mut self) {
        if let Some(tp) = self.tracer.take() {
            let _ = tp.shutdown_with_timeout(SHUTDOWN_TIMEOUT);
        }
        if let Some(mp) = self.meter.take() {
            let _ = mp.shutdown_with_timeout(SHUTDOWN_TIMEOUT);
        }
        if let Some(lp) = self.logger.take() {
            let _ = lp.shutdown_with_timeout(SHUTDOWN_TIMEOUT);
        }
    }
}
