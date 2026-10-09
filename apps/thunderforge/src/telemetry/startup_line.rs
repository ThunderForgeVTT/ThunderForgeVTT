//! The one `INFO` line that says where telemetry goes (FR-008, Appendix A.3).
//! `disclosure_tests.rs` holds these templates to the spec's words.

use thunderforge_server::telemetry::TelemetryStatus;

/// Appendix A.3, when `TELEMETRY=false`.
pub const OFF_LINE: &str =
    "telemetry: off (TELEMETRY=false). Nothing is sent anywhere. See docs/guides/telemetry.md";

/// Appendix A.3's text after the two destinations.
pub const ON_TAIL: &str = "Anonymous goes to the ThunderForge project; set OTEL_EXPORTER_OTLP_ENDPOINT and THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT to use your own collector, or TELEMETRY=false to send nothing. See docs/guides/telemetry.md";

/// Where a side's data goes, as the line names it. A server with
/// `OTEL_SDK_DISABLED=true` exports nothing, and says so.
const NOWHERE: &str = "nowhere";

pub fn startup_line(status: &TelemetryStatus) -> String {
    if !status.enabled {
        return OFF_LINE.to_string();
    }
    let server = status.server_endpoint.as_deref().unwrap_or(NOWHERE);
    let browser = if status.browser.enabled {
        status.browser.endpoint.as_str()
    } else {
        NOWHERE
    };
    format!(
        "telemetry: server → {server} ({}), browsers → {browser} ({}). {ON_TAIL}",
        status.server_tier_wire(),
        status.browser_tier_wire(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use thunderforge_server::telemetry::BrowserTelemetry;
    use thunderforge_telemetry_policy::{PROJECT_TELEMETRY_ENDPOINT, Tier};

    const MINE: &str = "https://otel.example.org";

    fn status(server: Option<(Tier, &str)>, browser: Option<(Tier, &str)>) -> TelemetryStatus {
        let mut s = TelemetryStatus::off_for_tests();
        s.enabled = true;
        if let Some((tier, endpoint)) = server {
            s.server_exporting = true;
            s.server_tier = tier;
            s.server_endpoint = Some(endpoint.to_string());
        }
        s.browser = BrowserTelemetry {
            enabled: true,
            ..BrowserTelemetry::off()
        };
        if let Some((tier, endpoint)) = browser {
            s.browser.tier = tier;
            s.browser.endpoint = endpoint.to_string();
        }
        s
    }

    #[test]
    fn on_with_nothing_set_names_the_project_twice() {
        let s = status(
            Some((Tier::Anonymous, PROJECT_TELEMETRY_ENDPOINT)),
            Some((Tier::Anonymous, PROJECT_TELEMETRY_ENDPOINT)),
        );
        assert_eq!(
            startup_line(&s),
            format!(
                "telemetry: server → https://telemetry.thunderforge.dev (anonymous), browsers → https://telemetry.thunderforge.dev (anonymous). {ON_TAIL}"
            )
        );
    }

    #[test]
    fn redirected_names_the_collector_and_full() {
        let s = status(Some((Tier::Operator, MINE)), Some((Tier::Operator, MINE)));
        assert_eq!(
            startup_line(&s),
            format!("telemetry: server → {MINE} (full), browsers → {MINE} (full). {ON_TAIL}")
        );
    }

    #[test]
    fn half_redirected_says_which_half() {
        let server_only = status(
            Some((Tier::Operator, MINE)),
            Some((Tier::Anonymous, PROJECT_TELEMETRY_ENDPOINT)),
        );
        assert!(startup_line(&server_only).starts_with(&format!(
            "telemetry: server → {MINE} (full), browsers → {PROJECT_TELEMETRY_ENDPOINT} (anonymous). "
        )));
        let browsers_only = status(
            Some((Tier::Anonymous, PROJECT_TELEMETRY_ENDPOINT)),
            Some((Tier::Operator, MINE)),
        );
        assert!(startup_line(&browsers_only).starts_with(&format!(
            "telemetry: server → {PROJECT_TELEMETRY_ENDPOINT} (anonymous), browsers → {MINE} (full). "
        )));
    }

    #[test]
    fn off_is_appendix_a3s_off_line() {
        assert_eq!(
            startup_line(&TelemetryStatus::off_for_tests()),
            "telemetry: off (TELEMETRY=false). Nothing is sent anywhere. See docs/guides/telemetry.md"
        );
    }

    #[test]
    fn a_disabled_sdk_says_the_server_sends_nothing() {
        let s = status(None, Some((Tier::Anonymous, PROJECT_TELEMETRY_ENDPOINT)));
        assert!(startup_line(&s).starts_with("telemetry: server → nowhere (off), browsers → "));
    }

    #[test]
    fn main_logs_it_exactly_once() {
        let main = include_str!("../main.rs");
        assert_eq!(main.matches("startup_line(").count(), 1);
        let at = main.find("startup_line(").unwrap();
        // The nearest macro before the call is the one it is logged with.
        let info = main[..at].rfind("tracing::info!(").expect("logged");
        assert!(
            !main[info..at].contains(';'),
            "the call is inside the INFO macro"
        );
    }
}
