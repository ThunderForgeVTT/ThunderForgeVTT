//! Where the server is willing to be pointed.
//!
//! # Why an admin-set URL still needs checking
//!
//! `userinfo_url` is settable through the admin GraphQL surface and is then
//! **fetched by the server**, with a bearer token attached. That makes it a
//! request the instance makes on somebody's behalf to an address somebody
//! chose — the shape of a server-side request forgery — and the interesting
//! targets are all on the inside: `169.254.169.254` for cloud instance
//! credentials, `127.0.0.1` for admin interfaces bound to loopback,
//! `10.0.0.0/8` for whatever else is on the network.
//!
//! "But only an administrator can set it" is a weaker argument than it sounds.
//! An administrator of a *ThunderForge instance* is not necessarily trusted
//! with the *infrastructure* it runs on — on anything hosted they are usually
//! two different people, and the whole point of this being self-hostable is
//! that somebody else runs it. An admin account is also a thing that gets
//! taken over, and this turns that into a credential-exfiltration primitive
//! rather than a ThunderForge problem.
//!
//! # What this does and does not promise
//!
//! It rejects the addresses that are *written down* as internal: literal
//! private, loopback and link-local IPs, and the hostnames cloud providers use
//! for metadata. It does **not** resolve DNS, so a hostname that resolves to
//! an internal address gets through, and a name that resolves differently on
//! the second lookup than the first (DNS rebinding) would defeat any check
//! made here rather than at connect time.
//!
//! That limit is deliberate and worth stating rather than hiding: this is a
//! guard against the realistic mistake and the casual abuse, not a claim that
//! SSRF is impossible. Closing the rest means resolving at request time and
//! refusing the socket, which belongs in the HTTP client rather than in a
//! validation function.

use url::{Host, Url};

/// Hostnames that mean "ask the platform for my credentials".
///
/// Matched exactly, case-insensitively. `169.254.169.254` is the address these
/// usually resolve to and is caught by the link-local rule below anyway; these
/// are here because a request made by name never becomes an IP for us to look
/// at.
const METADATA_HOSTS: &[&str] = &[
    "metadata.google.internal",
    "metadata.goog",
    "instance-data",
    "metadata",
];

/// Why a URL was refused.
///
/// Named reasons rather than a bare `false`: this is shown to an administrator
/// who has just typed something, and "that address is inside the network" is
/// actionable in a way that "invalid" is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UrlRefusal {
    NotAUrl,
    /// Only `http` and `https` are fetched. `file://`, `gopher://` and friends
    /// are how a fetch becomes a local read.
    UnsupportedScheme,
    /// No host at all — `http:///path`.
    NoHost,
    /// Loopback, private, link-local, or a known metadata name.
    InternalAddress,
}

impl UrlRefusal {
    pub fn message(self) -> &'static str {
        match self {
            Self::NotAUrl => "That is not a valid URL.",
            Self::UnsupportedScheme => "Only http and https addresses can be used.",
            Self::NoHost => "That URL has no host.",
            Self::InternalAddress => {
                "That address is inside this server's own network, so it cannot be \
                 used as a provider endpoint."
            }
        }
    }
}

/// May the server be pointed at `candidate`?
///
/// `allow_loopback` exists for one reason: a developer running a provider on
/// `127.0.0.1` during setup, and the e2e harness doing the same. It is a
/// parameter rather than a `cfg!(debug_assertions)` so the decision is made by
/// the caller that knows the context, and so this function stays testable in
/// both directions from one place.
pub fn check_outbound_url(candidate: &str, allow_loopback: bool) -> Result<(), UrlRefusal> {
    let url = Url::parse(candidate.trim()).map_err(|_| UrlRefusal::NotAUrl)?;

    if !matches!(url.scheme(), "http" | "https") {
        return Err(UrlRefusal::UnsupportedScheme);
    }

    let host = url.host().ok_or(UrlRefusal::NoHost)?;

    match host {
        Host::Domain(name) => {
            let lowered = name.to_ascii_lowercase();
            if METADATA_HOSTS.contains(&lowered.as_str()) {
                return Err(UrlRefusal::InternalAddress);
            }
            // `localhost` and anything under `.localhost` resolve to loopback
            // by specification (RFC 6761), so they are the same case as
            // `127.0.0.1` and are treated as such.
            let loops_back = lowered == "localhost" || lowered.ends_with(".localhost");
            if loops_back && !allow_loopback {
                return Err(UrlRefusal::InternalAddress);
            }
            Ok(())
        }
        Host::Ipv4(ip) => {
            if ip.is_loopback() {
                return if allow_loopback {
                    Ok(())
                } else {
                    Err(UrlRefusal::InternalAddress)
                };
            }
            // `is_private` covers 10/8, 172.16/12 and 192.168/16;
            // `is_link_local` covers 169.254/16, which is where every cloud
            // metadata service lives. `is_unspecified` is 0.0.0.0, which
            // routes to the local host on most stacks.
            if ip.is_private() || ip.is_link_local() || ip.is_unspecified() || ip.is_broadcast() {
                return Err(UrlRefusal::InternalAddress);
            }
            Ok(())
        }
        Host::Ipv6(ip) => {
            if ip.is_loopback() {
                return if allow_loopback {
                    Ok(())
                } else {
                    Err(UrlRefusal::InternalAddress)
                };
            }
            // Unique-local (fc00::/7) and link-local (fe80::/10) by hand:
            // the stable standard library has no predicate for either, and an
            // IPv6-only internal network is not a hypothetical.
            let segments = ip.segments();
            let unique_local = (segments[0] & 0xfe00) == 0xfc00;
            let link_local = (segments[0] & 0xffc0) == 0xfe80;
            if ip.is_unspecified() || unique_local || link_local {
                return Err(UrlRefusal::InternalAddress);
            }
            // An IPv4-mapped address is an IPv4 address wearing a hat, and
            // `::ffff:169.254.169.254` must not be a way around the rules
            // above.
            if let Some(v4) = ip.to_ipv4_mapped() {
                if v4.is_loopback() && !allow_loopback {
                    return Err(UrlRefusal::InternalAddress);
                }
                if v4.is_private() || v4.is_link_local() || v4.is_unspecified() {
                    return Err(UrlRefusal::InternalAddress);
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ordinary_provider_endpoint_is_allowed() {
        for url in [
            "https://accounts.google.com/o/oauth2/v2/auth",
            "https://github.com/login/oauth/authorize",
            "https://id.example.org:8443/userinfo",
        ] {
            assert_eq!(check_outbound_url(url, false), Ok(()), "{url}");
        }
    }

    /// The one that matters. Every cloud metadata service is on this address,
    /// and a fetch to it with a bearer token attached is how an SSRF becomes
    /// a set of infrastructure credentials.
    #[test]
    fn cloud_metadata_is_refused_by_address_and_by_name() {
        for url in [
            "http://169.254.169.254/latest/meta-data/iam/security-credentials/",
            "http://metadata.google.internal/computeMetadata/v1/",
            "http://[fe80::1]/",
            "http://metadata/",
        ] {
            assert_eq!(
                check_outbound_url(url, false),
                Err(UrlRefusal::InternalAddress),
                "{url}",
            );
        }
    }

    #[test]
    fn private_and_loopback_ranges_are_refused() {
        for url in [
            "http://10.0.0.5/userinfo",
            "http://172.16.4.1/userinfo",
            "http://192.168.1.1/userinfo",
            "http://127.0.0.1:9000/userinfo",
            "http://localhost:9000/userinfo",
            "http://0.0.0.0/userinfo",
            "http://[::1]/userinfo",
            "http://[fd00::1]/userinfo",
        ] {
            assert_eq!(
                check_outbound_url(url, false),
                Err(UrlRefusal::InternalAddress),
                "{url}",
            );
        }
    }

    /// An IPv4 address wearing an IPv6 hat is still an IPv4 address.
    #[test]
    fn ipv4_mapped_addresses_do_not_slip_past() {
        for url in [
            "http://[::ffff:169.254.169.254]/",
            "http://[::ffff:10.0.0.1]/",
            "http://[::ffff:127.0.0.1]/",
        ] {
            assert_eq!(
                check_outbound_url(url, false),
                Err(UrlRefusal::InternalAddress),
                "{url}",
            );
        }
    }

    /// A fetch of `file:///etc/passwd` is a local read with extra steps.
    #[test]
    fn only_http_and_https_are_fetched() {
        for url in [
            "file:///etc/passwd",
            "gopher://example.org/",
            "ftp://example.org/",
        ] {
            assert_eq!(
                check_outbound_url(url, false),
                Err(UrlRefusal::UnsupportedScheme),
                "{url}",
            );
        }
    }

    /// Loopback is allowed only where the caller says so — a developer running
    /// a provider locally, and the e2e harness's stub.
    #[test]
    fn loopback_is_allowed_only_when_the_caller_permits_it() {
        assert_eq!(
            check_outbound_url("http://127.0.0.1:31600/userinfo", true),
            Ok(())
        );
        assert_eq!(
            check_outbound_url("http://localhost:31600/userinfo", true),
            Ok(())
        );
        // Permitting loopback must not permit the rest of the inside.
        assert_eq!(
            check_outbound_url("http://169.254.169.254/", true),
            Err(UrlRefusal::InternalAddress),
        );
        assert_eq!(
            check_outbound_url("http://10.0.0.5/", true),
            Err(UrlRefusal::InternalAddress),
        );
    }

    #[test]
    fn nonsense_is_refused_as_nonsense() {
        assert_eq!(check_outbound_url("", false), Err(UrlRefusal::NotAUrl));
        assert_eq!(
            check_outbound_url("not a url at all", false),
            Err(UrlRefusal::NotAUrl),
        );
    }
}
