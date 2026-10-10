//! The harness for the gateway's tests: the router in process, a fake
//! upstream on `127.0.0.1:0` that records every request, an in-memory meter,
//! a hand-moved clock and a captured `tracing` subscriber.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use clap::Parser as _;
use opentelemetry::metrics::MeterProvider as _;
use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};
use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};
use tokio::sync::Notify;
use tower::ServiceExt as _;

use crate::metrics::Metrics;
use crate::{AppState, Config, app};

/// One request the fake upstream received, every byte of it.
#[derive(Debug, Clone)]
pub struct Received {
    pub path: String,
    pub headers: Vec<(String, Vec<u8>)>,
    pub body: Vec<u8>,
}

impl Received {
    /// The headers and the body, as one byte string to search.
    pub fn all_bytes(&self) -> Vec<u8> {
        let mut out = self.path.as_bytes().to_vec();
        for (k, v) in &self.headers {
            out.extend_from_slice(k.as_bytes());
            out.extend_from_slice(v);
        }
        out.extend_from_slice(&self.body);
        out
    }
}

#[derive(Clone)]
pub enum Mode {
    /// `200` with this body.
    Answer(Vec<u8>),
    Status(u16),
    /// Waits on the notify before answering `200`.
    Hold(Arc<Notify>),
}

#[derive(Clone)]
pub struct Fake {
    pub addr: SocketAddr,
    pub received: Arc<Mutex<Vec<Received>>>,
    pub mode: Arc<Mutex<Mode>>,
}

impl Fake {
    pub async fn start() -> Self {
        let received = Arc::new(Mutex::new(Vec::new()));
        let mode = Arc::new(Mutex::new(Mode::Answer(Vec::new())));
        let (r, m) = (received.clone(), mode.clone());
        let router = Router::new().fallback(move |req: Request| {
            let (r, m) = (r.clone(), m.clone());
            async move {
                let (parts, body) = req.into_parts();
                let body = axum::body::to_bytes(body, usize::MAX)
                    .await
                    .unwrap_or_default();
                r.lock().unwrap().push(Received {
                    path: parts.uri.path().to_owned(),
                    headers: parts
                        .headers
                        .iter()
                        .map(|(k, v)| (k.as_str().to_owned(), v.as_bytes().to_vec()))
                        .collect(),
                    body: body.to_vec(),
                });
                let mode = m.lock().unwrap().clone();
                match mode {
                    Mode::Answer(b) => (StatusCode::OK, b).into_response(),
                    Mode::Status(s) => StatusCode::from_u16(s).unwrap().into_response(),
                    Mode::Hold(n) => {
                        n.notified().await;
                        StatusCode::OK.into_response()
                    }
                }
            }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        Self {
            addr,
            received,
            mode,
        }
    }

    pub fn received(&self) -> Vec<Received> {
        self.received.lock().unwrap().clone()
    }

    pub fn set(&self, mode: Mode) {
        *self.mode.lock().unwrap() = mode;
    }
}

/// A `tracing` writer into a shared buffer.
#[derive(Clone, Default)]
pub struct Captured(pub Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct Harness {
    pub app: Router,
    pub fake: Fake,
    pub state: Arc<AppState>,
    exporter: InMemoryMetricExporter,
    provider: SdkMeterProvider,
    now: Arc<Mutex<Instant>>,
    pub logs: Captured,
    _log_guard: tracing::subscriber::DefaultGuard,
}

pub const PEER: &str = "10.1.2.3:40000";

impl Harness {
    pub async fn new(extra: &[&str]) -> Self {
        let logs = Captured::default();
        let writer = logs.clone();
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(move || writer.clone())
            .finish();
        let log_guard = tracing::subscriber::set_default(subscriber);

        let fake = Fake::start().await;
        let upstream = format!("http://{}", fake.addr);
        let mut args = vec!["thunderforge-telemetry-gateway", "--upstream", &upstream];
        args.extend_from_slice(extra);
        let config = Config::parse_from(args);

        let exporter = InMemoryMetricExporter::default();
        let provider = SdkMeterProvider::builder()
            .with_reader(PeriodicReader::builder(exporter.clone()).build())
            .build();
        let metrics = Metrics::new(&provider.meter("thunderforge-telemetry-gateway"));
        let now = Arc::new(Mutex::new(Instant::now()));
        let clock_now = now.clone();
        let clock = Arc::new(move || *clock_now.lock().unwrap());
        let state = AppState::new(config, metrics, clock).unwrap();
        Self {
            app: app(state.clone()),
            fake,
            state,
            exporter,
            provider,
            now,
            logs,
            _log_guard: log_guard,
        }
    }

    pub fn advance(&self, by: Duration) {
        *self.now.lock().unwrap() += by;
    }

    pub async fn send(&self, mut req: Request) -> Response {
        let peer: SocketAddr = PEER.parse().unwrap();
        req.extensions_mut().insert(ConnectInfo(peer));
        self.app.clone().oneshot(req).await.unwrap()
    }

    /// A counter's value for the data point whose attributes include `attr`.
    pub fn counter(&self, name: &str, attr: (&str, &str)) -> u64 {
        self.provider.force_flush().unwrap();
        let exported = self.exporter.get_finished_metrics().unwrap();
        let Some(last) = exported.last() else {
            return 0;
        };
        let mut total = 0;
        for scope in last.scope_metrics() {
            for metric in scope.metrics().filter(|m| m.name() == name) {
                if let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data() {
                    for point in sum.data_points() {
                        let hit = point
                            .attributes()
                            .any(|kv| kv.key.as_str() == attr.0 && kv.value.as_str() == attr.1);
                        if hit {
                            total += point.value();
                        }
                    }
                }
            }
        }
        total
    }

    /// Every exported metric, as text to search.
    pub fn metrics_text(&self) -> String {
        self.provider.force_flush().unwrap();
        format!("{:?}", self.exporter.get_finished_metrics().unwrap())
    }

    pub fn logs_text(&self) -> String {
        String::from_utf8_lossy(&self.logs.0.lock().unwrap()).into_owned()
    }
}

pub fn fixture(name: &str) -> Vec<u8> {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", name]
        .iter()
        .collect();
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A post of `body` to `path`, with these extra headers.
pub fn request(path: &str, content_type: &str, body: Vec<u8>, headers: &[(&str, &str)]) -> Request {
    let mut b = Request::post(path).header("content-type", content_type);
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    b.body(Body::from(body)).unwrap()
}

pub async fn body_bytes(res: Response) -> Vec<u8> {
    axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

pub const CHROME_UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.6778.85 Safari/537.36";
