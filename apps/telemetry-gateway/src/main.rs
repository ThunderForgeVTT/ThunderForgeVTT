//! `thunderforge-telemetry-gateway`: the flags, the gateway's own metrics,
//! and `axum::serve`. Everything else is in the library.

use std::net::SocketAddr;

use clap::Parser as _;
use opentelemetry::global;
use opentelemetry_otlp::{MetricExporter, WithExportConfig as _};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use thunderforge_telemetry_gateway::limits::system_clock;
use thunderforge_telemetry_gateway::metrics::Metrics;
use thunderforge_telemetry_gateway::{AppState, Config, app};

const SERVICE_NAME: &str = "thunderforge-telemetry-gateway";
const DEFAULT_OWN_ENDPOINT: &str = "http://otel-collector.monitoring:4318";

/// The gateway's own metrics, to the in-cluster collector, never the public
/// one. `TELEMETRY=false` turns this off, never the checks.
fn own_telemetry() -> Option<SdkMeterProvider> {
    if std::env::var("TELEMETRY").is_ok_and(|v| v.eq_ignore_ascii_case("false")) {
        return None;
    }
    let base = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_OWN_ENDPOINT.to_owned());
    let url = format!("{}/v1/metrics", base.trim_end_matches('/'));
    let exporter = match MetricExporter::builder()
        .with_http()
        .with_endpoint(url)
        .build()
    {
        Ok(e) => e,
        Err(err) => {
            tracing::warn!(%err, "no metric exporter; the gateway runs on without its own metrics");
            return None;
        }
    };
    let resource = Resource::builder_empty()
        .with_service_name(SERVICE_NAME)
        .with_attribute(opentelemetry::KeyValue::new(
            "service.version",
            env!("CARGO_PKG_VERSION"),
        ))
        .build();
    let provider = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(PeriodicReader::builder(exporter).build())
        .build();
    global::set_meter_provider(provider.clone());
    Some(provider)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::parse();
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    // Built before the runtime: the exporter's blocking client owns a
    // runtime of its own, which must not start inside ours.
    let provider = own_telemetry();
    let metrics = Metrics::new(&global::meter(SERVICE_NAME));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let listen = config.listen;
        let state = AppState::new(config, metrics, system_clock())?;
        let listener = tokio::net::TcpListener::bind(listen).await?;
        tracing::info!(%listen, "listening");
        axum::serve(
            listener,
            app(state).into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal())
        .await?;
        Ok::<_, Box<dyn std::error::Error>>(())
    })?;
    if let Some(provider) = provider {
        let _ = provider.shutdown();
    }
    Ok(())
}

/// Kubernetes stops a pod with SIGTERM; a terminal stops it with Ctrl-C.
/// Either one lets the requests in flight finish.
async fn shutdown_signal() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut term) => {
                term.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
    tracing::info!("shutting down");
}
