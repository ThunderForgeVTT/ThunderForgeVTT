//! One batch through the policy (FR-039 to FR-041): decode by content type,
//! check each resource and record, strip and count what is not listed, label
//! what is left, and say what was dropped in `partial_success`.

use std::time::Duration;

use opentelemetry_proto::tonic::collector::logs::v1::{
    ExportLogsPartialSuccess, ExportLogsServiceRequest, ExportLogsServiceResponse,
};
use opentelemetry_proto::tonic::collector::metrics::v1::{
    ExportMetricsPartialSuccess, ExportMetricsServiceRequest, ExportMetricsServiceResponse,
};
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTracePartialSuccess, ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use opentelemetry_proto::tonic::common::v1::{AnyValue, InstrumentationScope, KeyValue, any_value};
use opentelemetry_proto::tonic::metrics::v1::metric::Data;
use opentelemetry_proto::tonic::resource::v1::Resource;
use prost::Message as _;
use thunderforge_telemetry_policy::{
    CAP_DEFAULT, CAP_LOG_BODY, DropReason, GATEWAY_LABELS, Place, SERVICE_NAMES, Source,
    attribute_allowed, cap_for, client_version, instance_id_required, is_instance_id,
    metric_allowed,
};

use crate::limits::Limits;

const INSTANCE_ID: &str = "thunderforge.instance.id";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Traces,
    Logs,
    Metrics,
}

impl Signal {
    pub fn as_str(self) -> &'static str {
        match self {
            Signal::Traces => "traces",
            Signal::Logs => "logs",
            Signal::Metrics => "metrics",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Protobuf,
    Json,
}

impl Format {
    pub fn content_type(self) -> &'static str {
        match self {
            Format::Protobuf => "application/x-protobuf",
            Format::Json => "application/json",
        }
    }
}

/// The format a `Content-Type` names, parameters ignored.
pub fn format_for(content_type: Option<&str>) -> Option<Format> {
    let essence = content_type?.split(';').next()?.trim().to_ascii_lowercase();
    match essence.as_str() {
        "application/x-protobuf" => Some(Format::Protobuf),
        "application/json" => Some(Format::Json),
        _ => None,
    }
}

/// Where a request came from, reduced to the labels of FR-041. Nothing here
/// is an address.
#[derive(Debug, Clone)]
pub struct Context {
    pub source: Source,
    pub origin_host: Option<String>,
    pub ua_family: &'static str,
    pub ua_major: String,
    pub country: Option<String>,
}

/// A decoded batch.
#[derive(Debug, Clone, PartialEq)]
pub enum Batch {
    Traces(ExportTraceServiceRequest),
    Logs(ExportLogsServiceRequest),
    Metrics(ExportMetricsServiceRequest),
}

impl Batch {
    pub fn decode(signal: Signal, format: Format, body: &[u8]) -> Option<Batch> {
        fn de<T: prost::Message + Default + serde::de::DeserializeOwned>(
            format: Format,
            body: &[u8],
        ) -> Option<T> {
            match format {
                Format::Protobuf => T::decode(body).ok(),
                Format::Json => serde_json::from_slice(body).ok(),
            }
        }
        Some(match signal {
            Signal::Traces => Batch::Traces(de(format, body)?),
            Signal::Logs => Batch::Logs(de(format, body)?),
            Signal::Metrics => Batch::Metrics(de(format, body)?),
        })
    }

    pub fn encode_protobuf(&self) -> Vec<u8> {
        match self {
            Batch::Traces(r) => r.encode_to_vec(),
            Batch::Logs(r) => r.encode_to_vec(),
            Batch::Metrics(r) => r.encode_to_vec(),
        }
    }
}

/// What the policy made of a batch.
#[derive(Debug, Default)]
pub struct Outcome {
    /// The reasons anything was dropped, each counted once per request.
    pub reasons: Vec<DropReason>,
    /// Records dropped, for `partial_success.rejected_*`.
    pub rejected: i64,
    /// Records left to forward.
    pub kept: usize,
    /// Attributes removed for not being listed.
    pub stripped: u64,
    /// Resources that passed the content checks.
    pub checked_resources: usize,
    /// Of those, the ones over their instance's bucket, with the longest wait.
    pub instance_limited: usize,
    pub retry_after: Option<Duration>,
}

impl Outcome {
    fn drop(&mut self, reason: DropReason, records: usize) {
        if !self.reasons.contains(&reason) {
            self.reasons.push(reason);
        }
        self.rejected += i64::try_from(records).unwrap_or(i64::MAX);
    }

    /// Every resource that passed the content checks was over its bucket.
    pub fn all_instance_limited(&self) -> bool {
        self.checked_resources > 0 && self.instance_limited == self.checked_resources
    }

    pub fn message(&self) -> String {
        if self.reasons.is_empty() {
            return String::new();
        }
        let reasons: Vec<&str> = self.reasons.iter().map(DropReason::as_str).collect();
        format!("dropped by the telemetry gateway: {}", reasons.join(", "))
    }
}

/// Applies the policy to every resource and record of `batch`, in place.
pub fn apply(batch: &mut Batch, ctx: &Context, limits: &Limits) -> Outcome {
    let mut out = Outcome::default();
    match batch {
        Batch::Traces(r) => r.resource_spans.retain_mut(|rs| {
            let records = rs.scope_spans.iter().map(|s| s.spans.len()).sum();
            let Some(service) = resource(&mut rs.resource, records, ctx, limits, &mut out) else {
                return false;
            };
            for ss in &mut rs.scope_spans {
                out.stripped += scope(&mut ss.scope);
                ss.spans.retain_mut(|span| {
                    let large = span.name.chars().count() > CAP_DEFAULT
                        || too_large(&span.attributes)
                        || span.events.iter().any(|e| too_large(&e.attributes));
                    if large {
                        out.drop(DropReason::AttributeTooLarge, 1);
                        return false;
                    }
                    out.stripped += strip(&mut span.attributes, &service, Place::Span);
                    for event in &mut span.events {
                        out.stripped += strip(&mut event.attributes, &service, Place::SpanEvent);
                    }
                    for link in &mut span.links {
                        out.stripped += clear(&mut link.attributes);
                    }
                    out.kept += 1;
                    true
                });
            }
            true
        }),
        Batch::Logs(r) => r.resource_logs.retain_mut(|rl| {
            let records = rl.scope_logs.iter().map(|s| s.log_records.len()).sum();
            let Some(service) = resource(&mut rl.resource, records, ctx, limits, &mut out) else {
                return false;
            };
            for sl in &mut rl.scope_logs {
                out.stripped += scope(&mut sl.scope);
                sl.log_records.retain_mut(|record| {
                    let large = record
                        .body
                        .as_ref()
                        .is_some_and(|b| value_len(b) > CAP_LOG_BODY)
                        || record.event_name.chars().count() > CAP_DEFAULT
                        || too_large(&record.attributes);
                    if large {
                        out.drop(DropReason::AttributeTooLarge, 1);
                        return false;
                    }
                    out.stripped += strip(&mut record.attributes, &service, Place::LogRecord);
                    out.kept += 1;
                    true
                });
            }
            true
        }),
        Batch::Metrics(r) => r.resource_metrics.retain_mut(|rm| {
            let records = rm
                .scope_metrics
                .iter()
                .flat_map(|s| &s.metrics)
                .map(points)
                .sum();
            let Some(service) = resource(&mut rm.resource, records, ctx, limits, &mut out) else {
                return false;
            };
            for sm in &mut rm.scope_metrics {
                out.stripped += scope(&mut sm.scope);
                sm.metrics.retain_mut(|metric| {
                    if !metric_allowed(&metric.name) {
                        out.drop(DropReason::MetricName, points(metric));
                        return false;
                    }
                    metric.metadata.clear();
                    data_points(metric.data.as_mut(), &service, &mut out);
                    true
                });
            }
            true
        }),
    }
    out
}

/// Checks and labels one resource. `None` when it is dropped, with
/// `records`, everything it held, counted as rejected.
fn resource(
    resource: &mut Option<Resource>,
    records: usize,
    ctx: &Context,
    limits: &Limits,
    out: &mut Outcome,
) -> Option<String> {
    let r = resource.get_or_insert_with(Resource::default);
    let service = string_attr(&r.attributes, "service.name").unwrap_or_default();
    if !SERVICE_NAMES.contains(&service.as_str()) {
        out.drop(DropReason::UnknownService, records);
        return None;
    }
    let instance = match r.attributes.iter().find(|kv| kv.key == INSTANCE_ID) {
        Some(kv) => match kv.value.as_ref().and_then(as_str) {
            Some(id) if is_instance_id(id) => Some(id.to_owned()),
            _ => {
                out.drop(DropReason::InstanceId, records);
                return None;
            }
        },
        None if instance_id_required(&service, ctx.source) => {
            out.drop(DropReason::InstanceId, records);
            return None;
        }
        None => None,
    };
    if too_large(&r.attributes) {
        out.drop(DropReason::AttributeTooLarge, records);
        return None;
    }
    out.checked_resources += 1;
    if let Some(id) = &instance
        && let Err(wait) = limits.take_instance(id)
    {
        out.instance_limited += 1;
        out.retry_after = Some(out.retry_after.map_or(wait, |w| w.max(wait)));
        out.drop(DropReason::RateLimitedInstance, records);
        return None;
    }
    let version =
        client_version(string_attr(&r.attributes, "service.version").as_deref()).to_owned();
    // A sender's own label of a gateway name is overwritten, so it is not
    // counted as stripped. The instance id stays, checked.
    r.attributes
        .retain(|kv| kv.key == INSTANCE_ID || !GATEWAY_LABELS.contains(&kv.key.as_str()));
    out.stripped += strip(&mut r.attributes, &service, Place::Resource);
    let mut label = |key: &str, value: &str| r.attributes.push(string_kv(key, value));
    label("thunderforge.ingress", "public");
    label("thunderforge.source", ctx.source.as_str());
    if let Some(host) = &ctx.origin_host {
        label("thunderforge.origin.host", host);
    }
    label("thunderforge.client.version", &version);
    label("thunderforge.user_agent.family", ctx.ua_family);
    label("thunderforge.user_agent.major", &ctx.ua_major);
    if let Some(country) = &ctx.country {
        label("thunderforge.country", country);
    }
    Some(service)
}

fn data_points(data: Option<&mut Data>, service: &str, out: &mut Outcome) {
    macro_rules! points {
        ($points:expr, $exemplars:ident) => {{
            $points.retain_mut(|p| {
                if too_large(&p.attributes) {
                    out.drop(DropReason::AttributeTooLarge, 1);
                    return false;
                }
                out.stripped += strip(&mut p.attributes, service, Place::DataPoint);
                for e in &mut p.$exemplars {
                    out.stripped += clear(&mut e.filtered_attributes);
                }
                out.kept += 1;
                true
            })
        }};
        ($points:expr) => {{
            $points.retain_mut(|p| {
                if too_large(&p.attributes) {
                    out.drop(DropReason::AttributeTooLarge, 1);
                    return false;
                }
                out.stripped += strip(&mut p.attributes, service, Place::DataPoint);
                out.kept += 1;
                true
            })
        }};
    }
    match data {
        Some(Data::Gauge(g)) => points!(g.data_points, exemplars),
        Some(Data::Sum(s)) => points!(s.data_points, exemplars),
        Some(Data::Histogram(h)) => points!(h.data_points, exemplars),
        Some(Data::ExponentialHistogram(h)) => points!(h.data_points, exemplars),
        Some(Data::Summary(s)) => points!(s.data_points),
        None => {}
    }
}

fn points(metric: &opentelemetry_proto::tonic::metrics::v1::Metric) -> usize {
    match &metric.data {
        Some(Data::Gauge(g)) => g.data_points.len(),
        Some(Data::Sum(s)) => s.data_points.len(),
        Some(Data::Histogram(h)) => h.data_points.len(),
        Some(Data::ExponentialHistogram(h)) => h.data_points.len(),
        Some(Data::Summary(s)) => s.data_points.len(),
        None => 0,
    }
}

/// Removes the attributes not listed at `place`, and says how many.
fn strip(attrs: &mut Vec<KeyValue>, service: &str, place: Place) -> u64 {
    let before = attrs.len();
    attrs.retain(|kv| attribute_allowed(service, place, &kv.key));
    (before - attrs.len()) as u64
}

fn clear(attrs: &mut Vec<KeyValue>) -> u64 {
    let n = attrs.len() as u64;
    attrs.clear();
    n
}

/// A scope keeps its name and version; no list names its attributes.
fn scope(scope: &mut Option<InstrumentationScope>) -> u64 {
    scope.as_mut().map_or(0, |s| clear(&mut s.attributes))
}

fn too_large(attrs: &[KeyValue]) -> bool {
    attrs.iter().any(|kv| {
        kv.key.chars().count() > CAP_DEFAULT
            || kv
                .value
                .as_ref()
                .is_some_and(|v| value_len(v) > cap_for(&kv.key))
    })
}

/// The longest string or byte run in a value, nested ones included.
fn value_len(v: &AnyValue) -> usize {
    match &v.value {
        Some(any_value::Value::StringValue(s)) => s.chars().count(),
        Some(any_value::Value::BytesValue(b)) => b.len(),
        Some(any_value::Value::ArrayValue(a)) => a.values.iter().map(value_len).max().unwrap_or(0),
        Some(any_value::Value::KvlistValue(l)) => l
            .values
            .iter()
            .map(|kv| {
                kv.key
                    .chars()
                    .count()
                    .max(kv.value.as_ref().map_or(0, value_len))
            })
            .max()
            .unwrap_or(0),
        _ => 0,
    }
}

fn as_str(v: &AnyValue) -> Option<&str> {
    match &v.value {
        Some(any_value::Value::StringValue(s)) => Some(s),
        _ => None,
    }
}

fn string_attr(attrs: &[KeyValue], key: &str) -> Option<String> {
    attrs
        .iter()
        .find(|kv| kv.key == key)
        .and_then(|kv| kv.value.as_ref())
        .and_then(as_str)
        .map(str::to_owned)
}

fn string_kv(key: &str, value: &str) -> KeyValue {
    KeyValue {
        key: key.to_owned(),
        value: Some(AnyValue {
            value: Some(any_value::Value::StringValue(value.to_owned())),
        }),
        ..Default::default()
    }
}

/// The answer's body: our own drops and the collector's, together, in the
/// request's format.
pub fn answer(signal: Signal, format: Format, out: &Outcome, upstream: Option<&[u8]>) -> Vec<u8> {
    let ours = (out.rejected, out.message());
    macro_rules! answer {
        ($resp:ident, $partial:ident, $field:ident) => {{
            let theirs = upstream
                .and_then(|b| $resp::decode(b).ok())
                .and_then(|r| r.partial_success);
            let (mut rejected, mut message) = ours;
            if let Some(p) = theirs {
                rejected += p.$field;
                if !p.error_message.is_empty() {
                    if !message.is_empty() {
                        message.push_str("; ");
                    }
                    message.push_str(&p.error_message);
                }
            }
            let partial = (rejected > 0 || !message.is_empty()).then(|| $partial {
                $field: rejected,
                error_message: message,
            });
            let response = $resp {
                partial_success: partial,
            };
            match format {
                Format::Protobuf => response.encode_to_vec(),
                Format::Json => serde_json::to_vec(&response).unwrap_or_default(),
            }
        }};
    }
    match signal {
        Signal::Traces => answer!(
            ExportTraceServiceResponse,
            ExportTracePartialSuccess,
            rejected_spans
        ),
        Signal::Logs => answer!(
            ExportLogsServiceResponse,
            ExportLogsPartialSuccess,
            rejected_log_records
        ),
        Signal::Metrics => answer!(
            ExportMetricsServiceResponse,
            ExportMetricsPartialSuccess,
            rejected_data_points
        ),
    }
}
