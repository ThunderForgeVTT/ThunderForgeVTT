//! `TelemetrySettings::from_env` (data-model.md): read once at start. A change
//! needs a restart, by design (ADR-114).

use super::tier::{PROJECT_TELEMETRY_ENDPOINT, Tier, tier_for};
use std::collections::HashMap;
use thunderforge_server::telemetry::{BrowserTelemetry, TelemetryStatus};

const PER_SIGNAL: [&str; 3] = [
    "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
    "OTEL_EXPORTER_OTLP_METRICS_ENDPOINT",
    "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
];

#[derive(Clone, Debug, PartialEq)]
pub struct TelemetrySettings {
    /// `TELEMETRY`: false, 0, no or off in any case is off; anything else is on.
    pub enabled: bool,
    /// `OTEL_SDK_DISABLED=true` stops the server's export only.
    pub sdk_disabled: bool,
    pub server_endpoint: String,
    pub server_tier: Tier,
    pub browser_endpoint: String,
    pub browser_sample_rate: f64,
    pub browser_environment: String,
    pub browser_tier: Tier,
}

fn is_off(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "no" | "off"
    )
}

impl TelemetrySettings {
    /// From the process environment.
    pub fn from_env() -> Self {
        Self::from_map(&std::env::vars().collect())
    }

    /// From a map, so the tests never touch the process environment.
    pub fn from_map(env: &HashMap<String, String>) -> Self {
        let get = |k: &str| {
            env.get(k)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };

        let enabled = get("TELEMETRY").is_none_or(|v| !is_off(&v));
        let sdk_disabled = get("OTEL_SDK_DISABLED").is_some_and(|v| v.eq_ignore_ascii_case("true"));

        let general = get("OTEL_EXPORTER_OTLP_ENDPOINT");
        let per_signal: Vec<String> = PER_SIGNAL.iter().filter_map(|k| get(k)).collect();
        let server_endpoint = general
            .clone()
            .or_else(|| get(PER_SIGNAL[0]))
            .unwrap_or_else(|| PROJECT_TELEMETRY_ENDPOINT.to_string());
        // A mixed set never sends operator-tier data to the project: any
        // per-signal endpoint that is not the project's makes every signal
        // operator.
        let server_tier = if tier_for(&server_endpoint) == Tier::Operator
            || per_signal.iter().any(|e| tier_for(e) == Tier::Operator)
        {
            Tier::Operator
        } else {
            Tier::Anonymous
        };

        let browser_endpoint = get("THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT")
            .unwrap_or_else(|| PROJECT_TELEMETRY_ENDPOINT.to_string());
        let browser_sample_rate = get("THUNDERFORGE_BROWSER_TELEMETRY_SAMPLE_RATE")
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|r| r.is_finite())
            .map_or(1.0, |r| r.clamp(0.0, 1.0));
        let browser_environment = get("THUNDERFORGE_BROWSER_TELEMETRY_ENVIRONMENT")
            .unwrap_or_else(|| "self-hosted".to_string());
        let browser_tier = tier_for(&browser_endpoint);

        Self {
            enabled,
            sdk_disabled,
            server_endpoint,
            server_tier,
            browser_endpoint,
            browser_sample_rate,
            browser_environment,
            browser_tier,
        }
    }

    /// The providers are installed only when this is true.
    pub fn exporting(&self) -> bool {
        self.enabled && !self.sdk_disabled
    }

    pub fn browser(&self, instance_id: &str) -> BrowserTelemetry {
        BrowserTelemetry {
            enabled: self.enabled,
            endpoint: self.browser_endpoint.clone(),
            sample_rate: self.browser_sample_rate,
            environment: self.browser_environment.clone(),
            tier: self.browser_tier,
            instance_id: instance_id.to_string(),
        }
    }

    pub fn status(&self, instance_id: &str) -> TelemetryStatus {
        TelemetryStatus {
            enabled: self.enabled,
            server_exporting: self.exporting(),
            server_tier: self.server_tier,
            server_endpoint: self.exporting().then(|| self.server_endpoint.clone()),
            browser: self.browser(instance_id),
            instance_id: instance_id.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> TelemetrySettings {
        TelemetrySettings::from_map(
            &pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }

    #[test]
    fn telemetry_off_in_any_case() {
        for v in ["false", "FALSE", "0", "no", "No", "off", " OFF "] {
            assert!(!env(&[("TELEMETRY", v)]).enabled, "{v}");
        }
        for v in ["true", "1", "yes", "anything"] {
            assert!(env(&[("TELEMETRY", v)]).enabled, "{v}");
        }
        assert!(env(&[]).enabled);
    }

    #[test]
    fn the_default_is_anonymous_to_the_project() {
        let s = env(&[]);
        assert_eq!(s.server_endpoint, PROJECT_TELEMETRY_ENDPOINT);
        assert_eq!(s.server_tier, Tier::Anonymous);
        assert_eq!(s.browser_tier, Tier::Anonymous);
        assert!(s.exporting());
    }

    #[test]
    fn sdk_disabled_stops_export_only() {
        let s = env(&[("OTEL_SDK_DISABLED", "TRUE")]);
        assert!(s.enabled);
        assert!(!s.exporting());
        assert!(s.status("id").browser.enabled);
        assert_eq!(s.status("id").server_endpoint, None);
    }

    #[test]
    fn a_general_endpoint_redirects_with_full_detail() {
        let s = env(&[("OTEL_EXPORTER_OTLP_ENDPOINT", "http://otel-collector:4318")]);
        assert_eq!(s.server_endpoint, "http://otel-collector:4318");
        assert_eq!(s.server_tier, Tier::Operator);
    }

    #[test]
    fn the_traces_endpoint_is_used_when_there_is_no_general_one() {
        let s = env(&[(
            "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
            "https://otel.example.org/v1/traces",
        )]);
        assert_eq!(s.server_endpoint, "https://otel.example.org/v1/traces");
        assert_eq!(s.server_tier, Tier::Operator);
    }

    #[test]
    fn any_foreign_per_signal_endpoint_makes_every_signal_operator() {
        let s = env(&[
            ("OTEL_EXPORTER_OTLP_ENDPOINT", PROJECT_TELEMETRY_ENDPOINT),
            (
                "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                "https://loki.example.org",
            ),
        ]);
        assert_eq!(s.server_tier, Tier::Operator);
        let s = env(&[(
            "OTEL_EXPORTER_OTLP_METRICS_ENDPOINT",
            "https://telemetry.thunderforge.dev/",
        )]);
        assert_eq!(s.server_tier, Tier::Anonymous);
    }

    #[test]
    fn browser_variables() {
        let s = env(&[
            (
                "THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT",
                "https://otel.example.org",
            ),
            ("THUNDERFORGE_BROWSER_TELEMETRY_SAMPLE_RATE", "0.25"),
            ("THUNDERFORGE_BROWSER_TELEMETRY_ENVIRONMENT", "staging"),
        ]);
        let b = s.browser("iid");
        assert_eq!(b.endpoint, "https://otel.example.org");
        assert_eq!(b.sample_rate, 0.25);
        assert_eq!(b.environment, "staging");
        assert_eq!(b.tier, Tier::Operator);
        assert_eq!(b.instance_id, "iid");

        for (given, rate) in [("7", 1.0), ("-2", 0.0), ("nope", 1.0), ("NaN", 1.0)] {
            let s = env(&[("THUNDERFORGE_BROWSER_TELEMETRY_SAMPLE_RATE", given)]);
            assert_eq!(s.browser_sample_rate, rate, "{given}");
        }
        assert_eq!(env(&[]).browser("x").environment, "self-hosted");
    }

    #[test]
    fn off_turns_off_the_browser_too() {
        let s = env(&[("TELEMETRY", "false")]);
        assert_eq!(
            s.status("id").browser.served_json().to_string(),
            r#"{"enabled":false}"#
        );
        assert_eq!(s.status("id").server_tier_wire(), "off");
    }
}
