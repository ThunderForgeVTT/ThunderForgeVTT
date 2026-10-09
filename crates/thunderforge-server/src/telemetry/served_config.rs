//! The served browser config and the demo's `connect-src`
//! (contracts/served-config-and-csp.md). Both are pure, so a table tests them.

use axum::Router;
use axum::http::header;
use axum::routing::get;
use serde_json::{Value, json};
use thunderforge_telemetry_policy::{PROJECT_TELEMETRY_ENDPOINT, Tier};

/// What browsers are told. Built by the app, so this crate never decides a tier.
#[derive(Clone, Debug, PartialEq)]
pub struct BrowserTelemetry {
    pub enabled: bool,
    pub endpoint: String,
    pub sample_rate: f64,
    pub environment: String,
    pub tier: Tier,
    pub instance_id: String,
}

impl BrowserTelemetry {
    /// Off, as every test stack runs.
    pub fn off() -> Self {
        Self {
            enabled: false,
            endpoint: PROJECT_TELEMETRY_ENDPOINT.to_string(),
            sample_rate: 1.0,
            environment: "self-hosted".to_string(),
            tier: Tier::Anonymous,
            instance_id: String::new(),
        }
    }

    fn clamped_rate(&self) -> f64 {
        if self.sample_rate.is_finite() {
            self.sample_rate.clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// The body of `/telemetry.json`. Off is exactly `{"enabled":false}`.
    pub fn served_json(&self) -> Value {
        if !self.enabled {
            return json!({ "enabled": false });
        }
        json!({
            "enabled": true,
            "endpoint": self.endpoint,
            "sampleRate": self.clamped_rate(),
            "environment": self.environment,
            "tier": self.tier.as_str(),
            "instanceId": self.instance_id,
        })
    }

    /// The `Content-Security-Policy` value for the demo.
    pub fn connect_src(&self) -> String {
        match self.enabled.then(|| origin_of(&self.endpoint)).flatten() {
            Some(origin) => format!("connect-src 'self' data: blob: {origin}"),
            None => "connect-src 'self' data: blob:".to_string(),
        }
    }
}

/// `/telemetry.json` and `/demo/telemetry.json`: `200`, JSON, never cached.
/// The body is built once, at start; a change needs a restart (ADR-114).
pub fn router<S>(telemetry: &BrowserTelemetry) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let body = telemetry.served_json().to_string();
    let serve = move || {
        let body = body.clone();
        async move {
            (
                [
                    (header::CONTENT_TYPE, "application/json"),
                    (header::CACHE_CONTROL, "no-store"),
                ],
                body,
            )
        }
    };
    Router::new()
        .route("/telemetry.json", get(serve.clone()))
        .route("/demo/telemetry.json", get(serve))
}

/// `scheme://authority` of an http(s) URL, or nothing.
fn origin_of(endpoint: &str) -> Option<String> {
    let url = url::Url::parse(endpoint.trim()).ok()?;
    if url.scheme() != "https" && url.scheme() != "http" {
        return None;
    }
    let origin = url.origin().ascii_serialization();
    (origin != "null").then_some(origin)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(endpoint: &str, tier: Tier) -> BrowserTelemetry {
        BrowserTelemetry {
            enabled: true,
            endpoint: endpoint.to_string(),
            sample_rate: 1.0,
            environment: "self-hosted".to_string(),
            tier,
            instance_id: "4f6c1c2e-8a53-4d8e-9a3b-0e2b9e7c6d11".to_string(),
        }
    }

    #[test]
    fn off_has_no_other_keys() {
        let mut b = on("https://otel.example.org", Tier::Operator);
        b.enabled = false;
        assert_eq!(b.served_json().to_string(), r#"{"enabled":false}"#);
        assert_eq!(
            BrowserTelemetry::off().served_json().to_string(),
            r#"{"enabled":false}"#
        );
    }

    #[test]
    fn on_carries_every_field() {
        let b = on(PROJECT_TELEMETRY_ENDPOINT, Tier::Anonymous);
        assert_eq!(
            b.served_json(),
            json!({
                "enabled": true,
                "endpoint": "https://telemetry.thunderforge.dev",
                "sampleRate": 1.0,
                "environment": "self-hosted",
                "tier": "anonymous",
                "instanceId": "4f6c1c2e-8a53-4d8e-9a3b-0e2b9e7c6d11",
            })
        );
        let op = on("https://otel.example.org", Tier::Operator);
        assert_eq!(op.served_json()["tier"], "operator");
    }

    #[test]
    fn the_sample_rate_is_clamped() {
        for (given, served) in [(7.0, 1.0), (-1.0, 0.0), (0.25, 0.25), (f64::NAN, 1.0)] {
            let mut b = on(PROJECT_TELEMETRY_ENDPOINT, Tier::Anonymous);
            b.sample_rate = given;
            assert_eq!(b.served_json()["sampleRate"], served, "{given}");
        }
    }

    #[test]
    fn connect_src_has_the_contracts_three_rows() {
        assert_eq!(
            on(PROJECT_TELEMETRY_ENDPOINT, Tier::Anonymous).connect_src(),
            "connect-src 'self' data: blob: https://telemetry.thunderforge.dev"
        );
        assert_eq!(
            on("https://otel.example.org/otlp", Tier::Operator).connect_src(),
            "connect-src 'self' data: blob: https://otel.example.org"
        );
        assert_eq!(
            BrowserTelemetry::off().connect_src(),
            "connect-src 'self' data: blob:"
        );
    }

    #[test]
    fn an_endpoint_that_is_not_a_url_adds_no_origin() {
        assert_eq!(
            on("not a url", Tier::Anonymous).connect_src(),
            "connect-src 'self' data: blob:"
        );
        assert_eq!(
            on(
                "http://otel-collector.monitoring.svc.cluster.local:4318",
                Tier::Operator
            )
            .connect_src(),
            "connect-src 'self' data: blob: http://otel-collector.monitoring.svc.cluster.local:4318"
        );
    }
}
