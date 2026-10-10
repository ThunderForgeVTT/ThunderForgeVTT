//! The HTTP surface (contracts/telemetry-gateway.md): three intake paths and
//! `/healthz`, behind CORS for any origin without credentials and the tower
//! stack of R31, checked in the contract's order.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Body;
use axum::error_handling::HandleErrorLayer;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use thunderforge_telemetry_policy::{
    DropReason, TokenBucket, country, reduce_user_agent, source_for,
};
use tower::{BoxError, ServiceBuilder};
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::Config;
use crate::intake::{self, Batch, Context, Format, Signal};
use crate::limits::{Clock, Limits};
use crate::metrics::Metrics;
use crate::upstream::Upstream;

/// The largest body accepted.
pub const BODY_LIMIT: usize = 4 * 1024 * 1024;

pub struct AppState {
    pub config: Config,
    pub limits: Limits,
    pub metrics: Metrics,
    pub upstream: Upstream,
}

impl AppState {
    pub fn new(
        config: Config,
        metrics: Metrics,
        clock: Clock,
    ) -> Result<Arc<Self>, reqwest::Error> {
        Ok(Arc::new(Self {
            limits: Limits::new(&config, clock),
            upstream: Upstream::new(&config)?,
            metrics,
            config,
        }))
    }
}

/// The whole service, ready for `into_make_service_with_connect_info`.
///
/// The tower stack wraps the router as one service, so the concurrency limit
/// is one for the process, not one per route.
pub fn app(state: Arc<AppState>) -> Router {
    let inner = Router::new()
        .route("/v1/traces", post(traces))
        .route("/v1/logs", post(logs))
        .route("/v1/metrics", post(metrics))
        .route("/healthz", get(|| async { "ok" }))
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(RequestBodyLimitLayer::new(BODY_LIMIT))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            count_too_large,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), ip_gate))
        .with_state(state.clone());

    let metrics = state.metrics.clone();
    let service = ServiceBuilder::new()
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::POST])
                .allow_headers([header::CONTENT_TYPE])
                .max_age(Duration::from_secs(7200)),
        )
        // CORS builds its preflight answer from a default body, which the
        // trace layer's wrapped body has not got.
        .map_response(IntoResponse::into_response)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|req: &Request| {
                    tracing::info_span!("request", method = %req.method(), path = known_path(req.uri().path()))
                })
                .on_request(())
                .on_response(|res: &Response, latency: Duration, _: &tracing::Span| {
                    tracing::info!(
                        status = res.status().as_u16(),
                        latency_ms = u64::try_from(latency.as_millis()).unwrap_or(u64::MAX),
                        "answered"
                    );
                })
                .on_failure(()),
        )
        .layer(HandleErrorLayer::new(move |err: BoxError| {
            let metrics = metrics.clone();
            async move {
                // A request over tower's own timeout is one the collector
                // did not answer; anything else is the concurrency limit.
                let reason = if err.is::<tower::timeout::error::Elapsed>() {
                    DropReason::UpstreamError
                } else {
                    DropReason::Overloaded
                };
                metrics.dropped(reason);
                StatusCode::SERVICE_UNAVAILABLE
            }
        }))
        .load_shed()
        .concurrency_limit(state.config.concurrency.max(1))
        .timeout(state.config.request_timeout())
        .service(inner);
    Router::new()
        .fallback_service(service)
        .layer(middleware::from_fn(preflight_no_content))
}

/// The only paths a log line names; anything else is `other`, so no path a
/// sender chose is ever written down.
fn known_path(path: &str) -> &'static str {
    match path {
        "/v1/traces" => "/v1/traces",
        "/v1/logs" => "/v1/logs",
        "/v1/metrics" => "/v1/metrics",
        "/healthz" => "/healthz",
        _ => "other",
    }
}

/// The CORS layer answers a preflight `200`; the contract says `204`.
async fn preflight_no_content(req: Request, next: Next) -> Response {
    let preflight = req.method() == Method::OPTIONS;
    let mut res = next.run(req).await;
    if preflight && res.status() == StatusCode::OK {
        *res.status_mut() = StatusCode::NO_CONTENT;
    }
    res
}

/// The per-address bucket, before the body is read. The address is hashed
/// and dropped here.
async fn ip_gate(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    if req.method() != Method::POST {
        return next.run(req).await;
    }
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip());
    let ip: Option<IpAddr> =
        crate::limits::client_ip(req.headers(), peer, state.config.trusted_hops);
    if let Some(ip) = ip
        && let Err(wait) = state.limits.take_ip(ip)
    {
        state.metrics.dropped(DropReason::RateLimitedIp);
        return too_many(wait);
    }
    next.run(req).await
}

/// Every `413`, the body limit's or the handler's, counted once.
async fn count_too_large(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let res = next.run(req).await;
    if res.status() == StatusCode::PAYLOAD_TOO_LARGE {
        state.metrics.dropped(DropReason::BodyTooLarge);
    }
    res
}

fn too_many(wait: Duration) -> Response {
    let secs = TokenBucket::retry_after_secs(wait);
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(header::RETRY_AFTER, secs.to_string())],
    )
        .into_response()
}

async fn traces(state: State<Arc<AppState>>, req: Request) -> Response {
    intake(state, Signal::Traces, req).await
}

async fn logs(state: State<Arc<AppState>>, req: Request) -> Response {
    intake(state, Signal::Logs, req).await
}

async fn metrics(state: State<Arc<AppState>>, req: Request) -> Response {
    intake(state, Signal::Metrics, req).await
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

fn context(headers: &HeaderMap) -> Context {
    let (source, origin_host) = source_for(header_str(headers, "origin"));
    let (ua_family, ua_major) = reduce_user_agent(header_str(headers, "user-agent"));
    Context {
        source,
        origin_host,
        ua_family,
        ua_major,
        country: country(header_str(headers, "cf-ipcountry")).map(str::to_owned),
    }
}

async fn intake(State(state): State<Arc<AppState>>, signal: Signal, req: Request) -> Response {
    let (parts, body) = req.into_parts();
    let Some(format) = intake::format_for(header_str(&parts.headers, "content-type")) else {
        state.metrics.dropped(DropReason::Undecodable);
        return StatusCode::BAD_REQUEST.into_response();
    };
    // The limit layer stops the body at 4 MiB; a body that fails to read is
    // answered `413`, which `count_too_large` counts.
    let Ok(bytes) = axum::body::to_bytes(body, BODY_LIMIT).await else {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    };
    let ctx = context(&parts.headers);
    drop(parts);
    let Some(mut batch) = Batch::decode(signal, format, &bytes) else {
        state.metrics.dropped(DropReason::Undecodable);
        return StatusCode::BAD_REQUEST.into_response();
    };
    drop(bytes);

    let outcome = intake::apply(&mut batch, &ctx, &state.limits);
    state.metrics.stripped(signal, outcome.stripped);
    for reason in &outcome.reasons {
        state.metrics.dropped(*reason);
    }
    if outcome.kept == 0 {
        if outcome.all_instance_limited() {
            return too_many(outcome.retry_after.unwrap_or(Duration::from_secs(1)));
        }
        return ok(format, intake::answer(signal, format, &outcome, None));
    }

    let Some(slot) = state.upstream.try_slot() else {
        state.metrics.dropped(DropReason::Overloaded);
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let started = Instant::now();
    let answered = state
        .upstream
        .post(slot, signal, batch.encode_protobuf())
        .await;
    let seconds = started.elapsed().as_secs_f64();
    state.metrics.upstream(signal, answered.is_some(), seconds);
    let Some(upstream) = answered else {
        state.metrics.dropped(DropReason::UpstreamError);
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    state.metrics.accepted(signal, ctx.source);
    ok(
        format,
        intake::answer(signal, format, &outcome, Some(&upstream)),
    )
}

fn ok(format: Format, body: Vec<u8>) -> Response {
    let mut res = Response::new(Body::from(body));
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(format.content_type()),
    );
    res
}
