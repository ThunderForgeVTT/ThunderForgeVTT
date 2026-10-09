//! The gateway's checks and labels (FR-039 to FR-041, R29, R30).
//!
//! Every label is indicative: `Origin`, `User-Agent` and `CF-IPCountry` can be
//! forged by a sender that is not a browser.

/// Hosts whose browsers are the project's own pages.
const OWNER_HOSTS: &[&str] = &["thunderforge.dev", "vtt-dev.thunderforge.dev"];

/// Where a batch came from, by its `Origin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    OwnerSite,
    SelfHostedBrowser,
    Server,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::OwnerSite => "owner_site",
            Source::SelfHostedBrowser => "self_hosted_browser",
            Source::Server => "server",
        }
    }
}

/// Why the gateway dropped something. The only value of its `reason` label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DropReason {
    RateLimitedIp,
    RateLimitedInstance,
    BodyTooLarge,
    Undecodable,
    UnknownService,
    MetricName,
    AttributeTooLarge,
    InstanceId,
    Overloaded,
    UpstreamError,
}

impl DropReason {
    pub const ALL: [DropReason; 10] = [
        DropReason::RateLimitedIp,
        DropReason::RateLimitedInstance,
        DropReason::BodyTooLarge,
        DropReason::Undecodable,
        DropReason::UnknownService,
        DropReason::MetricName,
        DropReason::AttributeTooLarge,
        DropReason::InstanceId,
        DropReason::Overloaded,
        DropReason::UpstreamError,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            DropReason::RateLimitedIp => "rate_limited_ip",
            DropReason::RateLimitedInstance => "rate_limited_instance",
            DropReason::BodyTooLarge => "body_too_large",
            DropReason::Undecodable => "undecodable",
            DropReason::UnknownService => "unknown_service",
            DropReason::MetricName => "metric_name",
            DropReason::AttributeTooLarge => "attribute_too_large",
            DropReason::InstanceId => "instance_id",
            DropReason::Overloaded => "overloaded",
            DropReason::UpstreamError => "upstream_error",
        }
    }
}

/// A hyphenated UUID, `8-4-4-4-12` hex digits.
pub fn is_instance_id(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, n)| g.len() == n && g.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Whether a resource must carry an instance id. Only the project's own
/// landing and demo, served from its own site, have none.
pub fn instance_id_required(service: &str, source: Source) -> bool {
    !(source == Source::OwnerSite
        && matches!(service, "thunderforge-landing" | "thunderforge-demo"))
}

/// The source and the origin host for a request's `Origin` header.
///
/// No header is a server (`reqwest` sends none). `null`, from a sandboxed
/// frame or a `file:` page, is a browser with an `opaque` host.
pub fn source_for(origin: Option<&str>) -> (Source, Option<String>) {
    let Some(origin) = origin.map(str::trim) else {
        return (Source::Server, None);
    };
    let host = origin_host(origin);
    match host {
        Some(host) if OWNER_HOSTS.contains(&host.as_str()) => (Source::OwnerSite, Some(host)),
        Some(host) => (Source::SelfHostedBrowser, Some(host)),
        None => (Source::SelfHostedBrowser, Some("opaque".to_owned())),
    }
}

/// The host of an `Origin`: no scheme, no port, no path, lower case, at most
/// 253 characters. `None` when there is none to take.
fn origin_host(origin: &str) -> Option<String> {
    let (_, rest) = origin.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let host = if let Some(v6) = authority.strip_prefix('[') {
        format!("[{}]", v6.split(']').next().unwrap_or(""))
    } else {
        authority.split(':').next().unwrap_or("").to_owned()
    };
    if host.is_empty() || host == "[]" {
        return None;
    }
    let host = host.to_ascii_lowercase();
    Some(crate::truncate_chars(&host, 253).to_owned())
}

/// `service.version` when it looks like a release, else `unknown`.
/// `^\d+\.\d+\.\d+([-+][0-9A-Za-z.-]{1,64})?$`
pub fn client_version(service_version: Option<&str>) -> &str {
    let Some(v) = service_version else {
        return "unknown";
    };
    let (core, suffix) = match v.find(['-', '+']) {
        Some(i) => (&v[..i], Some(&v[i + 1..])),
        None => (v, None),
    };
    let numbers = core.split('.').collect::<Vec<_>>();
    let core_ok = numbers.len() == 3
        && numbers
            .iter()
            .all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    let suffix_ok = suffix.is_none_or(|s| {
        (1..=64).contains(&s.len())
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
    });
    if core_ok && suffix_ok { v } else { "unknown" }
}

/// A user agent reduced to a family and a major version (R30), by ordered
/// substring rules. Anything else is `other` and `unknown`.
pub fn reduce_user_agent(ua: Option<&str>) -> (&'static str, String) {
    let Some(ua) = ua else {
        return ("other", "unknown".to_owned());
    };
    let rules: [(&str, &'static str); 5] = [
        ("Edg/", "edge"),
        ("OPR/", "opera"),
        ("SamsungBrowser/", "samsung"),
        ("Firefox/", "firefox"),
        ("Chrome/", "chrome"),
    ];
    for (token, family) in rules {
        if let Some(i) = ua.find(token) {
            return (family, major_after(&ua[i + token.len()..]));
        }
    }
    if ua.contains("Safari/")
        && let Some(i) = ua.find("Version/")
    {
        return ("safari", major_after(&ua[i + "Version/".len()..]));
    }
    let lower = ua.to_ascii_lowercase();
    for token in ["opentelemetry", "otel-otlp", "reqwest"] {
        if let Some(i) = lower.find(token) {
            let after = &lower[i..];
            let version = after.split_once('/').map(|(_, v)| major_after(v));
            return ("otel-rust", version.unwrap_or_else(|| "unknown".to_owned()));
        }
    }
    ("other", "unknown".to_owned())
}

fn major_after(rest: &str) -> String {
    let digits: String = rest
        .chars()
        .take_while(char::is_ascii_digit)
        .take(6)
        .collect();
    if digits.is_empty() {
        "unknown".to_owned()
    } else {
        digits
    }
}

/// The country from `CF-IPCountry`: two capital letters, not `XX` (unknown)
/// and not `T1` (Tor). Never looked up from an address.
pub fn country(cf_ipcountry: Option<&str>) -> Option<&str> {
    let c = cf_ipcountry?.trim();
    (c.len() == 2 && c.bytes().all(|b| b.is_ascii_uppercase()) && c != "XX").then_some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_reasons() {
        let names: Vec<&str> = DropReason::ALL.iter().map(DropReason::as_str).collect();
        assert_eq!(
            names,
            [
                "rate_limited_ip",
                "rate_limited_instance",
                "body_too_large",
                "undecodable",
                "unknown_service",
                "metric_name",
                "attribute_too_large",
                "instance_id",
                "overloaded",
                "upstream_error",
            ]
        );
    }

    #[test]
    fn instance_ids() {
        assert!(is_instance_id("4f6c1c2e-8a53-4d8e-9a3b-0e2b9e7c6d11"));
        assert!(is_instance_id("4F6C1C2E-8A53-4D8E-9A3B-0E2B9E7C6D11"));
        assert!(!is_instance_id("4f6c1c2e8a534d8e9a3b0e2b9e7c6d11"));
        assert!(!is_instance_id("4f6c1c2e-8a53-4d8e-9a3b-0e2b9e7c6d1"));
        assert!(!is_instance_id("zf6c1c2e-8a53-4d8e-9a3b-0e2b9e7c6d11"));
        assert!(!is_instance_id(""));
    }

    #[test]
    fn instance_id_is_required_except_for_the_owners_pages() {
        assert!(!instance_id_required(
            "thunderforge-landing",
            Source::OwnerSite
        ));
        assert!(!instance_id_required(
            "thunderforge-demo",
            Source::OwnerSite
        ));
        assert!(instance_id_required("thunderforge-web", Source::OwnerSite));
        assert!(instance_id_required(
            "thunderforge-demo",
            Source::SelfHostedBrowser
        ));
        assert!(instance_id_required("thunderforge", Source::Server));
    }

    #[test]
    fn sources() {
        let s = |o| source_for(o);
        assert_eq!(s(None), (Source::Server, None));
        assert_eq!(
            s(Some("https://thunderforge.dev")),
            (Source::OwnerSite, Some("thunderforge.dev".into()))
        );
        assert_eq!(
            s(Some("https://vtt-dev.thunderforge.dev")),
            (Source::OwnerSite, Some("vtt-dev.thunderforge.dev".into()))
        );
        assert_eq!(
            s(Some("https://Game.Example.org:8443")),
            (Source::SelfHostedBrowser, Some("game.example.org".into()))
        );
        assert_eq!(
            s(Some("null")),
            (Source::SelfHostedBrowser, Some("opaque".into()))
        );
        assert_eq!(
            s(Some("https://evil.thunderforge.dev.example")).0,
            Source::SelfHostedBrowser
        );
        assert_eq!(Source::OwnerSite.as_str(), "owner_site");
        assert_eq!(Source::SelfHostedBrowser.as_str(), "self_hosted_browser");
        assert_eq!(Source::Server.as_str(), "server");
    }

    #[test]
    fn versions() {
        assert_eq!(client_version(Some("0.1.0")), "0.1.0");
        assert_eq!(client_version(Some("1.2.3-rc.1")), "1.2.3-rc.1");
        assert_eq!(client_version(Some("1.2.3+abc")), "1.2.3+abc");
        assert_eq!(client_version(Some("1.2")), "unknown");
        assert_eq!(client_version(Some("1.2.3 drop table")), "unknown");
        assert_eq!(client_version(Some("1.2.3-")), "unknown");
        assert_eq!(client_version(None), "unknown");
    }

    #[test]
    fn user_agents() {
        let fixtures = [
            (
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
                "chrome",
                "131",
            ),
            (
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.2903.70",
                "edge",
                "131",
            ),
            (
                "Mozilla/5.0 (X11; Linux x86_64; rv:133.0) Gecko/20100101 Firefox/133.0",
                "firefox",
                "133",
            ),
            (
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Safari/605.1.15",
                "safari",
                "18",
            ),
            (
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 OPR/115.0.0.0",
                "opera",
                "115",
            ),
            (
                "Mozilla/5.0 (Linux; Android 14; SM-S918B) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/26.0 Chrome/122.0.0.0 Mobile Safari/537.36",
                "samsung",
                "26",
            ),
            ("OTel-OTLP-Exporter-Rust/0.33.0", "otel-rust", "0"),
            ("curl/8.5.0", "other", "unknown"),
        ];
        for (ua, family, major) in fixtures {
            assert_eq!(
                reduce_user_agent(Some(ua)),
                (family, major.to_owned()),
                "{ua}"
            );
        }
        assert_eq!(reduce_user_agent(None), ("other", "unknown".to_owned()));
    }

    #[test]
    fn countries() {
        assert_eq!(country(Some("DE")), Some("DE"));
        assert_eq!(country(Some("XX")), None);
        assert_eq!(country(Some("T1")), None);
        assert_eq!(country(Some("de")), None);
        assert_eq!(country(Some("DEU")), None);
        assert_eq!(country(None), None);
    }
}
