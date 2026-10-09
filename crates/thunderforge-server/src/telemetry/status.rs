//! What the admin query and the startup line read (FR-026). Immutable after
//! start: a change needs a restart, by design (ADR-114).

use super::served_config::BrowserTelemetry;
use thunderforge_telemetry_policy::Tier;

#[derive(Clone, Debug, PartialEq)]
pub struct TelemetryStatus {
    /// `TELEMETRY`.
    pub enabled: bool,
    /// False when `TELEMETRY=false` or `OTEL_SDK_DISABLED=true`.
    pub server_exporting: bool,
    pub server_tier: Tier,
    /// `None` when not exporting.
    pub server_endpoint: Option<String>,
    pub browser: BrowserTelemetry,
    pub instance_id: String,
}

impl TelemetryStatus {
    /// Everything off, as every test stack runs (FR-036).
    pub fn off_for_tests() -> Self {
        Self {
            enabled: false,
            server_exporting: false,
            server_tier: Tier::Anonymous,
            server_endpoint: None,
            browser: BrowserTelemetry::off(),
            instance_id: String::new(),
        }
    }

    /// `anonymous`, `full` or `off`, as Appendix A.3 words it.
    pub fn server_tier_wire(&self) -> &'static str {
        if self.server_exporting {
            self.server_tier.wire_name()
        } else {
            "off"
        }
    }

    pub fn browser_tier_wire(&self) -> &'static str {
        if self.browser.enabled {
            self.browser.tier.wire_name()
        } else {
            "off"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_reads_off_on_both_sides() {
        let s = TelemetryStatus::off_for_tests();
        assert_eq!(s.server_tier_wire(), "off");
        assert_eq!(s.browser_tier_wire(), "off");
    }

    #[test]
    fn operator_reads_full() {
        let mut s = TelemetryStatus::off_for_tests();
        s.server_exporting = true;
        s.server_tier = Tier::Operator;
        assert_eq!(s.server_tier_wire(), "full");
        s.server_tier = Tier::Anonymous;
        assert_eq!(s.server_tier_wire(), "anonymous");
    }
}
