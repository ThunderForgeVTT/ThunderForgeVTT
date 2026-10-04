//! Spec 040 US4: whether this instance can tell somebody something, and what
//! happens to the message when it cannot.
//!
//! # The shape of it
//!
//! - [`MailTransport`] — the seam. One trait, three implementations: SMTP for
//!   the product, [`Unconfigured`] for an instance that has not been told
//!   where to send, and `capture::CapturingTransport` for tests.
//! - [`outbox`] — the durable record. **Nothing calls
//!   [`MailTransport::send`] except the sender**, and a feature that wants to
//!   send something calls [`outbox::enqueue`] instead.
//! - [`schedule`] — the background sender, modelled on
//!   [`crate::lore_sync::schedule`], which is the only other retry loop in
//!   this product and therefore the only retry curve worth having a second of.
//! - [`graphql`] — the operator's surface: is mail available, prove it, and
//!   what has this instance failed to send.
//!
//! # Why the transport is a trait and not an `Option<SmtpClient>`
//!
//! An `Option` puts "did we remember to check?" at every call site, and the
//! answer to "why not" is then reconstructed at each one. A transport that
//! *refuses with a reason* puts the answer in one place and makes it a
//! sentence an operator can act on. This is not a new pattern here either:
//! `AppState` already holds `adjudicator: Arc<dyn SessionAdjudicator>` with a
//! local implementation and a remote one, and `test_support` already swaps it.
//!
//! # Why the seam is a [`MailSeam`] rather than a bare `Arc<dyn MailTransport>`
//!
//! Because FR-007 says a settings change takes effect with no restart, and
//! `AppState` is cloned into every request — a transport built once at startup
//! and stored in it would still be using the SMTP host an operator corrected
//! ten minutes ago. So the seam holds an *override* (which tests set, and
//! nothing else ever does) and otherwise builds the transport from the
//! settings as they resolve right now, exactly as
//! [`crate::settings::resolver`] does for everything else and for the reason
//! its own docs give: "a cached setting is a setting that keeps its old value
//! after somebody changes it."
//!
//! # What this module deliberately does not have
//!
//! No templates, no HTML, no per-feature message catalogue. This feature owns
//! *whether* a message can be sent; specs 035, 037 and 039 own what theirs
//! say. `body_text` is plain text and a richer form is a later feature's
//! problem, not a column nobody fills in yet.

use std::sync::Arc;

use crate::settings::registry::Capability;
use crate::settings::resolver::{Settings, required_for};

pub mod capture;
pub mod graphql;
pub mod outbox;
pub mod schedule;
pub mod smtp;

/// One message, as the transport sees it. Plain text, deliberately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingMessage {
    pub to: String,
    pub subject: String,
    pub body_text: String,
}

/// Whether this instance can send, and what is missing when it cannot.
///
/// `missing` names **setting keys**, never a host, never a credential and
/// never a fragment of one. Readiness renders the same keys from the same
/// declarations (FR-027).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Ready,
    Unconfigured { missing: Vec<&'static str> },
}

impl Availability {
    pub fn is_ready(&self) -> bool {
        matches!(self, Availability::Ready)
    }

    pub fn missing(&self) -> &[&'static str] {
        match self {
            Availability::Ready => &[],
            Availability::Unconfigured { missing } => missing,
        }
    }

    /// What an operator is told is limited while mail is unavailable.
    ///
    /// Sentences rather than keys, because this answers "what can my instance
    /// not do" and the answer to that is never a variable name. Empty when
    /// mail is available — FR-025's positive assertion.
    pub fn limited_features(&self) -> Vec<String> {
        if self.is_ready() {
            return Vec::new();
        }
        vec![
            "Messages this instance means to send are held rather than sent. \
             Nothing is discarded, and configuring mail releases them."
                .to_string(),
            "An operator cannot prove delivery works before a feature depends \
             on it."
                .to_string(),
        ]
    }
}

/// Why a message did not go, and whether trying again could help.
///
/// `reason` is operator-facing prose naming what to fix. It never contains a
/// password, a fragment of one, or its length (FR-014) — and that is enforced
/// by [`smtp::scrub`] rather than by remembering, because the one string that
/// leaks a credential is the one nobody thought about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryFailure {
    pub reason: String,
    /// False for a refusal that trying again cannot fix — a permanent SMTP
    /// code, or an instance with no mail server configured. The outbox uses
    /// this to decide between the backoff curve and `failed`.
    pub retryable: bool,
}

impl DeliveryFailure {
    pub fn permanent(reason: impl Into<String>) -> Self {
        DeliveryFailure {
            reason: reason.into(),
            retryable: false,
        }
    }

    pub fn transient(reason: impl Into<String>) -> Self {
        DeliveryFailure {
            reason: reason.into(),
            retryable: true,
        }
    }
}

/// Somewhere a message can be handed to. The seam this whole module exists
/// around.
#[async_trait::async_trait]
pub trait MailTransport: Send + Sync {
    async fn send(&self, message: &OutgoingMessage) -> Result<(), DeliveryFailure>;

    /// What readiness reports. Never a host, never a credential.
    fn availability(&self) -> Availability;
}

/// The transport for an instance that has not been told where to send.
///
/// It refuses, and the refusal names the settings to fill in. That refusal is
/// what lands in `mail_outbox.last_failure_reason` for a `blocked` row, which
/// is how FR-015's "not silently discarded" becomes something an operator can
/// actually read.
pub struct Unconfigured {
    missing: Vec<&'static str>,
}

impl Unconfigured {
    pub fn new(missing: Vec<&'static str>) -> Self {
        Unconfigured { missing }
    }

    /// The sentence an operator sees. Names keys, never values.
    pub fn reason(&self) -> String {
        format!(
            "This instance has no mail server configured, so nothing can be \
             sent yet. Set {} and the message will be sent.",
            join_keys(&self.missing)
        )
    }
}

#[async_trait::async_trait]
impl MailTransport for Unconfigured {
    async fn send(&self, _message: &OutgoingMessage) -> Result<(), DeliveryFailure> {
        // Not retryable: no amount of waiting configures a mail server. The
        // outbox reads this as `blocked` rather than `failed` — see
        // `outbox::classify`, where the difference between "we cannot yet" and
        // "we tried and gave up" is decided in one place.
        Err(DeliveryFailure::permanent(self.reason()))
    }

    fn availability(&self) -> Availability {
        Availability::Unconfigured {
            missing: self.missing.clone(),
        }
    }
}

/// `` `a`, `b` and `c` `` — the list an operator reads in a refusal.
pub fn join_keys(keys: &[&'static str]) -> String {
    let quoted: Vec<String> = keys.iter().map(|k| format!("`{k}`")).collect();
    match quoted.split_last() {
        None => "the mail settings".to_string(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

/// Which declared `mail.*` settings are not usable as they stand.
///
/// Derived from the declaration list rather than from a second list here, so a
/// setting added to `settings::registry` is a setting this notices. `enabled`
/// is checked separately because it is `Optional` — a mail server that is
/// fully described but switched off is a deliberate state, not a gap.
pub fn missing_mail_settings(settings: &Settings) -> Vec<&'static str> {
    let mut missing: Vec<&'static str> = required_for(Capability::SendMail)
        .filter(|d| !settings.is_set(d.key))
        .map(|d| d.key)
        .collect();

    let enabled = settings
        .value("mail.enabled")
        .is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes"));
    if !enabled {
        missing.insert(0, "mail.enabled");
    }
    missing
}

/// How the rest of the server gets a transport.
///
/// Held by `AppState`. Empty in production — the transport is built from the
/// settings on each use — and holding an override in tests, which is what lets
/// a test assert what *would* have been sent without sending it.
#[derive(Clone, Default)]
pub struct MailSeam {
    override_transport: Option<Arc<dyn MailTransport>>,
}

impl MailSeam {
    /// Production. Nothing is held; the transport comes from the settings.
    pub fn from_settings() -> Self {
        MailSeam::default()
    }

    /// Tests, and only tests. The one construction that pins a transport.
    pub fn overridden(transport: Arc<dyn MailTransport>) -> Self {
        MailSeam {
            override_transport: Some(transport),
        }
    }

    /// The transport to use for the settings as they resolve right now.
    pub fn transport(&self, settings: &Settings) -> Arc<dyn MailTransport> {
        if let Some(overridden) = &self.override_transport {
            return Arc::clone(overridden);
        }
        let missing = missing_mail_settings(settings);
        if missing.is_empty() {
            match smtp::SmtpTransport::from_settings(settings) {
                Ok(transport) => Arc::new(transport),
                // Every required setting is present and the transport still
                // could not be described — a port that is not a number, a
                // security mode that is not one of the three. Reported as the
                // key to look at rather than as a panic at startup (FR-028).
                Err(key) => Arc::new(Unconfigured::new(vec![key])),
            }
        } else {
            Arc::new(Unconfigured::new(missing))
        }
    }
}

/// The transport for the settings this instance resolves right now.
pub async fn transport_now(
    state: &crate::state::AppState,
) -> Result<Arc<dyn MailTransport>, String> {
    let settings = crate::settings::resolver::resolve_all(state).await?;
    Ok(state.mail.transport(&settings))
}

/// The Mailpit-backed level. Ignored by default; the file says how to run it.
#[cfg(test)]
#[path = "smtp_integration_tests.rs"]
mod smtp_integration_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_from_an_unconfigured_instance_names_the_settings_and_no_values() {
        let transport = Unconfigured::new(vec!["mail.host", "mail.from_address"]);
        let reason = transport.reason();
        assert!(reason.contains("`mail.host`"), "{reason}");
        assert!(reason.contains("`mail.from_address`"), "{reason}");
        assert!(!reason.contains("password"), "{reason}");
    }

    #[test]
    fn an_unconfigured_transport_refuses_permanently_rather_than_pretending() {
        let transport = Unconfigured::new(vec!["mail.host"]);
        let failure = tokio_test_block(transport.send(&OutgoingMessage {
            to: "someone@example.invalid".to_string(),
            subject: "s".to_string(),
            body_text: "b".to_string(),
        }));
        let failure = failure.expect_err("an unconfigured instance must not report success");
        assert!(!failure.retryable);
        assert!(failure.reason.contains("`mail.host`"));
    }

    /// A capability with nothing missing says so positively. An empty list
    /// that means "we did not check" and an empty list that means "everything
    /// is set" must not render alike (FR-025 scenario 3).
    #[test]
    fn availability_reports_limits_only_when_there_are_any() {
        assert!(Availability::Ready.limited_features().is_empty());
        let unconfigured = Availability::Unconfigured {
            missing: vec!["mail.host"],
        };
        assert!(!unconfigured.limited_features().is_empty());
        assert!(
            unconfigured
                .limited_features()
                .iter()
                .any(|s| s.contains("Nothing is discarded"))
        );
    }

    #[test]
    fn keys_are_joined_as_prose_rather_than_as_a_debug_list() {
        assert_eq!(join_keys(&["mail.host"]), "`mail.host`");
        assert_eq!(
            join_keys(&["mail.host", "mail.port"]),
            "`mail.host` and `mail.port`"
        );
        assert_eq!(
            join_keys(&["mail.host", "mail.port", "mail.security"]),
            "`mail.host`, `mail.port` and `mail.security`"
        );
    }

    /// The one-line executor the two synchronous tests above need. Cheaper
    /// than a `#[tokio::test]` for a future that never yields.
    fn tokio_test_block<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a current-thread runtime")
            .block_on(future)
    }
}
