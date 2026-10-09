//! Which tier an endpoint gets (FR-006). This is the only place that decides.

/// Where anonymous telemetry goes: the ThunderForge project's public intake.
pub const PROJECT_TELEMETRY_ENDPOINT: &str = "https://telemetry.thunderforge.dev";

const PROJECT_HOST: &str = "telemetry.thunderforge.dev";

/// How much a destination is told.
///
/// `Anonymous` is the project's endpoint: an allow-list of attributes and no
/// logs but redacted errors. `Operator` is anyone else's collector, which gets
/// full detail because the operator already owns that data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    Anonymous,
    Operator,
}

impl Tier {
    /// The name in the served config and in resource attributes.
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Anonymous => "anonymous",
            Tier::Operator => "operator",
        }
    }

    /// The name on the admin query and in Appendix A's text.
    pub fn wire_name(self) -> &'static str {
        match self {
            Tier::Anonymous => "anonymous",
            Tier::Operator => "full",
        }
    }
}

/// The tier for an endpoint.
///
/// Anonymous when the endpoint normalises to [`PROJECT_TELEMETRY_ENDPOINT`]
/// (scheme and host in any case, an explicit `:443`, a trailing slash), or
/// when it does not parse as a URL at all: a typo only ever sends less.
/// Anything else is another destination, so it is `Operator`.
pub fn tier_for(endpoint: &str) -> Tier {
    let endpoint = endpoint.trim();
    let Some((scheme, rest)) = endpoint.split_once("://") else {
        return Tier::Anonymous;
    };
    let (authority, path) = match rest.find(['/', '?', '#']) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    if scheme.is_empty() || authority.is_empty() || scheme.contains(char::is_whitespace) {
        return Tier::Anonymous;
    }
    if !scheme.eq_ignore_ascii_case("https") {
        return Tier::Operator;
    }
    let host_port = authority.to_ascii_lowercase();
    let host = host_port.strip_suffix(":443").unwrap_or(&host_port);
    if host == PROJECT_HOST && (path.is_empty() || path == "/") {
        Tier::Anonymous
    } else {
        Tier::Operator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// contracts/served-config-and-csp.md, "The tier's normalisation".
    #[test]
    fn normalisation_table() {
        let table = [
            ("https://telemetry.thunderforge.dev", Tier::Anonymous),
            ("https://telemetry.thunderforge.dev/", Tier::Anonymous),
            ("https://TELEMETRY.thunderforge.dev", Tier::Anonymous),
            ("https://telemetry.thunderforge.dev:443", Tier::Anonymous),
            ("HTTPS://telemetry.thunderforge.dev:443/", Tier::Anonymous),
            ("not a url", Tier::Anonymous),
            ("", Tier::Anonymous),
            ("http://telemetry.thunderforge.dev", Tier::Operator),
            ("https://telemetry.thunderforge.dev/v1", Tier::Operator),
            ("https://otel.example.org", Tier::Operator),
            (
                "http://otel-collector.monitoring.svc.cluster.local:4318",
                Tier::Operator,
            ),
        ];
        for (input, want) in table {
            assert_eq!(tier_for(input), want, "tier_for({input:?})");
        }
    }

    #[test]
    fn lookalikes_are_other_destinations() {
        for input in [
            "https://telemetry.thunderforge.dev.evil.example",
            "https://telemetry.thunderforge.dev:8443",
            "https://telemetry.thunderforge.dev?x=1",
            "https://user@telemetry.thunderforge.dev",
        ] {
            assert_eq!(tier_for(input), Tier::Operator, "tier_for({input:?})");
        }
    }

    #[test]
    fn the_project_endpoint_is_anonymous() {
        assert_eq!(tier_for(PROJECT_TELEMETRY_ENDPOINT), Tier::Anonymous);
    }

    #[test]
    fn names() {
        assert_eq!(Tier::Anonymous.as_str(), "anonymous");
        assert_eq!(Tier::Operator.as_str(), "operator");
        assert_eq!(Tier::Operator.wire_name(), "full");
        assert_eq!(Tier::Anonymous.wire_name(), "anonymous");
    }
}
