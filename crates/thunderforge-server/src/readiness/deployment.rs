//! The two ways an instance can be running unsafely that no setting describes.
//!
//! # Why these are not declarations
//!
//! Everything else readiness reports is a setting: a declared key, resolved
//! from the environment or a row, editable or fixed. These two are facts about
//! how the process was *started* — `Config`, read once from the environment
//! before the database is reachable — and neither can be a row, because one of
//! them is the key the rows are encrypted with.
//!
//! So `assess` reports `Capability::DeploySafely` as it reports a capability
//! nothing declares for (available, no gaps), and `apply` adds what `Config`
//! says. The gaps have the same shape as every other gap and are rendered by
//! the same screens.
//!
//! # What it found before it existed
//!
//! `compose.yml` supplies a default `THUNDERFORGE_SECRET` so that `docker
//! compose up` works with nothing set, and `Config::from_env` falls back to
//! another when the variable is absent. Both are in this repository. An
//! operator who followed the README onto a public address was running with
//! session cookies and stored credentials keyed by a published value, with
//! cookies sent over plain HTTP, and nothing on any screen said so.
//!
//! # It reports, it does not refuse
//!
//! An instance on a laptop is *meant* to run like this, and FR-028 forbids
//! readiness becoming a refusal to start. The secret gap is reported wherever
//! the instance runs because the instance cannot tell a laptop from a server
//! with certainty; the cookie gap only once a public address that is not this
//! machine has been configured, because on `localhost` over HTTP a `Secure`
//! cookie is simply a sign-in that does not work.
//!
//! # It never names the secret
//!
//! Not the value, not a prefix, not its length — the rule the module above
//! states, applied to the one value where breaking it would matter most. The
//! only thing read from it is a yes or a no.

use super::{CapabilityReport, Gap};
use crate::config::Config;
use crate::settings::registry::Capability;

/// What `Config` and the environment say about how this process was started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentFacts {
    pub secret_is_shipped_default: bool,
    pub secure_cookies: bool,
    /// `THUNDERFORGE_PUBLIC_URL`, when set.
    pub public_url: Option<String>,
}

impl DeploymentFacts {
    pub fn of(config: &Config) -> Self {
        Self {
            secret_is_shipped_default: config.secret_is_shipped_default(),
            secure_cookies: config.secure_cookies,
            public_url: crate::settings::registry::read_env("THUNDERFORGE_PUBLIC_URL"),
        }
    }
}

/// Whether a configured public address is this machine talking to itself.
///
/// Unparseable is treated as local: this decides whether to raise a warning,
/// and a warning raised because somebody mistyped a URL would be about the
/// wrong thing.
fn is_this_machine(public_url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(public_url) else {
        return true;
    };
    match parsed.host() {
        Some(url::Host::Domain(domain)) => {
            let domain = domain.to_ascii_lowercase();
            domain == "localhost" || domain.ends_with(".localhost")
        }
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        None => true,
    }
}

/// The gaps, from the facts. Pure.
pub fn deployment_gaps(facts: &DeploymentFacts) -> Vec<Gap> {
    let mut gaps = Vec::new();

    if facts.secret_is_shipped_default {
        gaps.push(Gap {
            setting_key: "deployment.secret".to_string(),
            env_var: Some("THUNDERFORGE_SECRET".to_string()),
            what_to_set: "This instance is running with the secret it shipped with, which is \
                          published in the project's source. Generate one of your own (for \
                          example `openssl rand -base64 64`), set it where the server is \
                          started, and restart."
                .to_string(),
            what_is_limited: "Anybody who has read the source can forge a session cookie for \
                              this instance and decrypt the credentials it stores. Changing \
                              the secret signs everybody out, and stored provider credentials \
                              and second factors have to be set again."
                .to_string(),
        });
    }

    let public = facts
        .public_url
        .as_deref()
        .is_some_and(|url| !is_this_machine(url));
    if public && !facts.secure_cookies {
        gaps.push(Gap {
            setting_key: "deployment.secure_cookies".to_string(),
            env_var: Some("THUNDERFORGE_SECURE_COOKIES".to_string()),
            what_to_set: "This instance has a public address and its session cookies are not \
                          marked secure. Serve it over HTTPS, set the variable to `true` \
                          where the server is started, and restart."
                .to_string(),
            what_is_limited: "A browser will send the session cookie over an unencrypted \
                              connection, where anybody on the path can read it and sign in \
                              as that person."
                .to_string(),
        });
    }

    gaps
}

/// Add the deployment gaps to the capability they belong to.
pub fn apply(capabilities: &mut [CapabilityReport], facts: &DeploymentFacts) {
    let key = Capability::DeploySafely.key();
    let Some(capability) = capabilities.iter_mut().find(|c| c.key == key) else {
        return;
    };
    capability.gaps.extend(deployment_gaps(facts));
    capability.available = capability.gaps.is_empty();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(default_secret: bool, secure: bool, url: Option<&str>) -> DeploymentFacts {
        DeploymentFacts {
            secret_is_shipped_default: default_secret,
            secure_cookies: secure,
            public_url: url.map(str::to_string),
        }
    }

    fn keys(facts: &DeploymentFacts) -> Vec<String> {
        deployment_gaps(facts)
            .into_iter()
            .map(|g| g.setting_key)
            .collect()
    }

    #[test]
    fn the_shipped_secret_is_a_gap_wherever_the_instance_runs() {
        assert_eq!(
            keys(&facts(true, true, None)),
            vec!["deployment.secret".to_string()]
        );
        assert!(keys(&facts(false, true, Some("https://play.example.org"))).is_empty());
    }

    #[test]
    fn insecure_cookies_are_a_gap_only_on_a_public_address() {
        for local in [
            None,
            Some("http://localhost:42080"),
            Some("http://127.0.0.1:42080/"),
            Some("http://[::1]:42080"),
            Some("http://LOCALHOST"),
        ] {
            assert!(
                keys(&facts(false, false, local)).is_empty(),
                "an instance on this machine was told to mark its cookies secure: {local:?}"
            );
        }

        for public in [
            "https://play.example.org",
            "http://192.168.1.20:42080",
            // A host that merely starts with the word is not this machine.
            "https://localhost.example.org",
        ] {
            assert_eq!(
                keys(&facts(false, false, Some(public))),
                vec!["deployment.secure_cookies".to_string()],
                "{public}"
            );
            assert!(keys(&facts(false, true, Some(public))).is_empty());
        }
    }

    /// The rule the whole report is written to, on the one value where it
    /// matters most.
    #[test]
    fn no_gap_names_the_secret() {
        let config = Config {
            secret: "c2VjcmV0LXZhbHVlLXRoYXQtbXVzdC1uZXZlci1iZS1wcmludGVk".to_string(),
            data_path: String::new(),
            secure_cookies: false,
        };
        let mut shipped = facts(true, false, Some("https://play.example.org"));
        shipped.secret_is_shipped_default = true;
        for gap in deployment_gaps(&shipped) {
            let text = format!("{gap:?}");
            assert!(!text.contains(&config.secret));
            assert!(
                !text.contains("dGh1bmRlcmZvcmdl"),
                "a gap quoted the shipped secret: {text}"
            );
        }
        assert!(!config.secret_is_shipped_default());
    }

    #[test]
    fn the_gaps_land_on_their_capability_and_make_it_unavailable() {
        let mut capabilities = vec![CapabilityReport {
            key: Capability::DeploySafely.key(),
            label: Capability::DeploySafely.label(),
            available: true,
            applicable: true,
            gaps: Vec::new(),
        }];

        apply(&mut capabilities, &facts(false, true, None));
        assert!(capabilities[0].available);

        apply(
            &mut capabilities,
            &facts(true, false, Some("https://play.example.org")),
        );
        assert!(!capabilities[0].available);
        assert_eq!(capabilities[0].gaps.len(), 2);
    }
}
