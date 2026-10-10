//! The flags of the contract's Configuration table, each with an
//! environment fallback.

use std::net::SocketAddr;
use std::time::Duration;

use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "thunderforge-telemetry-gateway",
    version,
    about = "ThunderForge's public telemetry intake: checks, labels and limits OTLP batches before the public collector"
)]
pub struct Config {
    /// Where to listen.
    #[arg(long, env = "TELEMETRY_GATEWAY_LISTEN", default_value = "0.0.0.0:4318")]
    pub listen: SocketAddr,
    /// The public collector's OTLP/HTTP receiver.
    #[arg(
        long,
        env = "TELEMETRY_GATEWAY_UPSTREAM",
        default_value = "http://otel-collector-public.monitoring:4319"
    )]
    pub upstream: String,
    /// Which `X-Forwarded-For` entry, from the right, is the client.
    #[arg(long, env = "TELEMETRY_GATEWAY_TRUSTED_HOPS", default_value_t = 1)]
    pub trusted_hops: usize,
    /// Requests per second one address may send.
    #[arg(long, env = "TELEMETRY_GATEWAY_IP_RATE", default_value_t = 5.0)]
    pub ip_rate: f64,
    /// Requests one address may send at once.
    #[arg(long, env = "TELEMETRY_GATEWAY_IP_BURST", default_value_t = 60)]
    pub ip_burst: u32,
    /// Resources per second one instance may send.
    #[arg(long, env = "TELEMETRY_GATEWAY_INSTANCE_RATE", default_value_t = 20.0)]
    pub instance_rate: f64,
    /// Resources one instance may send at once.
    #[arg(long, env = "TELEMETRY_GATEWAY_INSTANCE_BURST", default_value_t = 200)]
    pub instance_burst: u32,
    /// Address buckets held at most.
    #[arg(long, env = "TELEMETRY_GATEWAY_MAX_IP_KEYS", default_value_t = 100_000)]
    pub max_ip_keys: usize,
    /// Instance buckets held at most.
    #[arg(
        long,
        env = "TELEMETRY_GATEWAY_MAX_INSTANCE_KEYS",
        default_value_t = 100_000
    )]
    pub max_instance_keys: usize,
    /// Requests in progress at most; the next is answered `503` at once.
    #[arg(long, env = "TELEMETRY_GATEWAY_CONCURRENCY", default_value_t = 256)]
    pub concurrency: usize,
    /// Posts to the collector in flight at most.
    #[arg(
        long,
        env = "TELEMETRY_GATEWAY_UPSTREAM_IN_FLIGHT",
        default_value_t = 64
    )]
    pub upstream_in_flight: usize,
    /// The collector's total timeout, in milliseconds (connect: 2000).
    #[arg(
        long,
        env = "TELEMETRY_GATEWAY_UPSTREAM_TIMEOUT_MS",
        default_value_t = 5000
    )]
    pub upstream_timeout_ms: u64,
}

/// The collector's connect timeout.
pub const UPSTREAM_CONNECT_TIMEOUT: Duration = Duration::from_millis(2000);

impl Config {
    pub fn upstream_timeout(&self) -> Duration {
        Duration::from_millis(self.upstream_timeout_ms)
    }

    /// Tower's own timeout: the collector's, with room to answer.
    pub fn request_timeout(&self) -> Duration {
        self.upstream_timeout() + Duration::from_secs(5)
    }
}
