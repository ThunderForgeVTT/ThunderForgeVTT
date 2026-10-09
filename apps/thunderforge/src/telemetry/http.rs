//! Spec 086 T042: the HTTP server span, and its duration (FR-004, R17).
//!
//! The span is at INFO on `SPAN_TARGET`, so the env filter passes it and the
//! Bunyan layer prints no START or END line for it (`bunyan.rs`). Its
//! `trace_id` and `span_id` are recorded only when OpenTelemetry gave the
//! span a context, so with `TELEMETRY=false` a log line is what it was.

use axum::extract::{MatchedPath, Request};
use axum::http::{HeaderMap, Method};
use axum::middleware::Next;
use axum::response::Response;
use opentelemetry::KeyValue;
use opentelemetry::trace::TraceContextExt as _;
use std::time::Instant;
use thunderforge_server::telemetry::SPAN_TARGET;
use thunderforge_server::telemetry::instruments::{Recorders, recorders};
use tower_http::classify::{ServerErrorsAsFailures, SharedClassifier};
use tower_http::trace::{MakeSpan, OnRequest, OnResponse, TraceLayer};
use tracing::Span;
use tracing::field::Empty;
use tracing_opentelemetry::OpenTelemetrySpanExt as _;

/// What a request is labelled when no route matched it.
pub const UNMATCHED: &str = "unmatched";

/// The `TraceLayer` the router uses. Failures still log as they did.
pub fn layer()
-> TraceLayer<SharedClassifier<ServerErrorsAsFailures>, HttpSpan, RecordIds, RecordStatus> {
    TraceLayer::new_for_http()
        .make_span_with(HttpSpan)
        .on_request(RecordIds)
        .on_response(RecordStatus)
}

/// The route template, never the path: `/worlds/{id}`, not the id.
pub fn route<B>(request: &axum::http::Request<B>) -> &str {
    request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or(UNMATCHED)
}

/// The method, from a closed set, so a made-up verb is not a new series.
pub fn method(method: &Method) -> &'static str {
    match *method {
        Method::GET => "GET",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::PATCH => "PATCH",
        Method::DELETE => "DELETE",
        Method::HEAD => "HEAD",
        Method::OPTIONS => "OPTIONS",
        _ => "other",
    }
}

pub fn status_class(status: u16) -> &'static str {
    match status {
        100..=199 => "1xx",
        200..=299 => "2xx",
        300..=399 => "3xx",
        400..=499 => "4xx",
        _ => "5xx",
    }
}

struct Headers<'a>(&'a HeaderMap);

impl opentelemetry::propagation::Extractor for Headers<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }
    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct HttpSpan;

impl<B> MakeSpan<B> for HttpSpan {
    fn make_span(&mut self, request: &axum::http::Request<B>) -> Span {
        let route = route(request);
        let method = method(request.method());
        let span = tracing::info_span!(
            target: SPAN_TARGET,
            "http.request",
            otel.name = %format!("HTTP {method} {route}"),
            otel.kind = "server",
            http.route = %route,
            http.request.method = method,
            http.response.status_code = Empty,
            trace_id = Empty,
            span_id = Empty,
        );
        // The browser's `traceparent` makes this span its child (US7).
        let parent = opentelemetry::global::get_text_map_propagator(|p| {
            p.extract(&Headers(request.headers()))
        });
        if parent.span().span_context().is_valid() {
            let _ = span.set_parent(parent);
        }
        span
    }
}

/// Records the ids once OpenTelemetry has given the span a context.
#[derive(Clone, Copy, Debug, Default)]
pub struct RecordIds;

impl<B> OnRequest<B> for RecordIds {
    fn on_request(&mut self, _request: &axum::http::Request<B>, span: &Span) {
        record_ids(span);
    }
}

pub fn record_ids(span: &Span) {
    let cx = span.context();
    let ids = cx.span().span_context().clone();
    if ids.is_valid() {
        span.record("trace_id", ids.trace_id().to_string());
        span.record("span_id", ids.span_id().to_string());
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RecordStatus;

impl<B> OnResponse<B> for RecordStatus {
    fn on_response(self, response: &Response<B>, _latency: std::time::Duration, span: &Span) {
        span.record("http.response.status_code", response.status().as_u16());
    }
}

/// `thunderforge.http.server.duration{route, method, status_class}`. A
/// middleware rather than `OnResponse`, which never sees the request.
pub async fn record_duration(request: Request, next: Next) -> Response {
    let Some(r) = recorders() else {
        return next.run(request).await;
    };
    let route = route(&request).to_string();
    let method = method(request.method());
    let started = Instant::now();
    let response = next.run(request).await;
    observe(r, route, method, response.status().as_u16(), started);
    response
}

pub fn observe(r: &Recorders, route: String, method: &'static str, status: u16, started: Instant) {
    r.http_duration.record(
        started.elapsed().as_secs_f64(),
        &[
            KeyValue::new("route", route),
            KeyValue::new("method", method),
            KeyValue::new("status_class", status_class(status)),
        ],
    );
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
