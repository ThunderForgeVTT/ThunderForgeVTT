//! What the anonymous tier lets leave (contracts/server-instruments.md).
//!
//! Two pieces, and nothing else decides what a span or a log record holds on
//! that tier:
//!
//! - [`AllowListSpanProcessor`] wraps the batch processor and cuts every
//!   ended span down to the policy crate's lists;
//! - [`ServerErrorLayer`] is the only log source: one `server.error` record
//!   per `ERROR` event, redacted, and no other field of the event read.

use opentelemetry::logs::{AnyValue, LogRecord, Logger, Severity};
use opentelemetry::trace::Status;
use opentelemetry::{Context, KeyValue};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::{Span, SpanData, SpanProcessor};
use std::time::{Duration, SystemTime};
use thunderforge_server::telemetry::redact::redact;
use thunderforge_telemetry_policy::{
    ANONYMOUS_SPAN_ATTRIBUTES, CAP_MESSAGE, CAP_STACK, SERVER_SPAN_EVENT_ATTRIBUTES, truncate_chars,
};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context as LayerContext, Layer};

/// Redacted, then cut to `max` characters.
fn clean(text: &str, max: usize) -> String {
    truncate_chars(&redact(text), max).to_string()
}

/// Wraps the batch processor; every span is scrubbed before it is queued.
#[derive(Debug)]
pub struct AllowListSpanProcessor<P> {
    inner: P,
}

impl<P> AllowListSpanProcessor<P> {
    pub fn new(inner: P) -> Self {
        Self { inner }
    }
}

/// The span as the anonymous tier sends it: its name, status and timing, the
/// allowed attributes, no links, and only the `exception` events, redacted.
pub fn scrub(span: &mut SpanData) {
    span.attributes
        .retain(|kv| ANONYMOUS_SPAN_ATTRIBUTES.contains(&kv.key.as_str()));
    span.dropped_attributes_count = 0;
    span.links.links.clear();
    span.links.dropped_count = 0;
    span.events.events.retain(|e| e.name == "exception");
    span.events.dropped_count = 0;
    for event in &mut span.events.events {
        event
            .attributes
            .retain(|kv| SERVER_SPAN_EVENT_ATTRIBUTES.contains(&kv.key.as_str()));
        for kv in &mut event.attributes {
            let max = match kv.key.as_str() {
                "exception.stacktrace" => CAP_STACK,
                _ => CAP_MESSAGE,
            };
            *kv = KeyValue::new(kv.key.clone(), clean(&kv.value.as_str(), max));
        }
        event.dropped_attributes_count = 0;
    }
    if let Status::Error { description } = &span.status {
        span.status = Status::error(clean(description, CAP_MESSAGE));
    }
}

impl<P: SpanProcessor> SpanProcessor for AllowListSpanProcessor<P> {
    fn on_start(&self, span: &mut Span, cx: &Context) {
        self.inner.on_start(span, cx);
    }

    fn on_end(&self, mut span: SpanData) {
        scrub(&mut span);
        self.inner.on_end(span);
    }

    fn force_flush(&self) -> OTelSdkResult {
        self.inner.force_flush()
    }

    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        self.inner.shutdown_with_timeout(timeout)
    }

    fn set_resource(&mut self, resource: &Resource) {
        self.inner.set_resource(resource);
    }
}

/// Reads an event's `message` and `error.type`, and nothing else.
#[derive(Default)]
struct ErrorFields {
    message: String,
    error_type: Option<String>,
}

impl Visit for ErrorFields {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "message" => self.message = value.to_string(),
            "error.type" => self.error_type = Some(value.to_string()),
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "message" => self.message = format!("{value:?}"),
            "error.type" => self.error_type = Some(format!("{value:?}")),
            _ => {}
        }
    }
}

/// A captured backtrace reduced to `crate::path:line` frames of our own
/// crates. Empty unless `RUST_BACKTRACE` is set.
pub fn reduce_backtrace(text: &str) -> String {
    let mut frames = Vec::new();
    let mut current: Option<&str> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("at ") {
            if let Some(func) = current.take() {
                let line_no = rest.rsplit(':').nth(1).unwrap_or("");
                frames.push(format!("{func}:{line_no}"));
            }
        } else if let Some((idx, func)) = line.split_once(": ")
            && idx.chars().all(|c| c.is_ascii_digit())
        {
            current = func.starts_with("thunderforge").then_some(func);
        }
    }
    truncate_chars(&frames.join("\n"), CAP_STACK).to_string()
}

/// Turns each `ERROR` event into one `server.error` log record.
pub struct ServerErrorLayer<L> {
    logger: L,
}

impl<L> ServerErrorLayer<L> {
    pub fn new(logger: L) -> Self {
        Self { logger }
    }
}

impl<S, L> Layer<S> for ServerErrorLayer<L>
where
    S: Subscriber,
    L: Logger + Send + Sync + 'static,
{
    fn on_event(&self, event: &Event<'_>, _ctx: LayerContext<'_, S>) {
        let meta = event.metadata();
        // The SDK's own complaints never become telemetry about telemetry.
        if *meta.level() != Level::ERROR || meta.target().starts_with("opentelemetry") {
            return;
        }
        let mut fields = ErrorFields::default();
        event.record(&mut fields);
        let message = clean(&fields.message, CAP_MESSAGE);
        let error_type = fields
            .error_type
            .map_or_else(|| meta.target().to_string(), |t| clean(&t, CAP_MESSAGE));
        let backtrace = std::backtrace::Backtrace::capture();
        let stack = match backtrace.status() {
            std::backtrace::BacktraceStatus::Captured => reduce_backtrace(&backtrace.to_string()),
            _ => String::new(),
        };

        let mut record = self.logger.create_log_record();
        record.set_event_name("server.error");
        record.set_timestamp(SystemTime::now());
        record.set_severity_number(Severity::Error);
        record.set_severity_text("ERROR");
        record.set_body(AnyValue::from(message.clone()));
        record.add_attribute("event.name", "server.error");
        record.add_attribute("error.type", error_type);
        record.add_attribute("error.message", message);
        record.add_attribute("error.stack", stack);
        self.logger.emit(record);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backtrace_keeps_only_our_frames_without_paths() {
        let text = "   0: std::backtrace::Backtrace::capture\n             at /rustc/abc/library/std/src/backtrace.rs:296:22\n   1: thunderforge_server::graphql::thing\n             at /home/someone/ThunderForgeVTT/crates/thunderforge-server/src/graphql/thing.rs:42:9\n   2: tokio::runtime::task\n             at /home/someone/.cargo/registry/tokio/src/runtime.rs:10:1\n";
        assert_eq!(
            reduce_backtrace(text),
            "thunderforge_server::graphql::thing:42"
        );
    }

    #[test]
    fn an_empty_backtrace_is_empty() {
        assert_eq!(reduce_backtrace(""), "");
    }
}
