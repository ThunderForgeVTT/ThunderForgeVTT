//! The transport that actually sends, and the one place a failure becomes a
//! sentence.
//!
//! # Why `lettre` with rustls
//!
//! `lettre` is the only maintained Rust SMTP client with a native async Tokio
//! transport, but the TLS choice is the decision that mattered (research.md
//! § R7): this product's first-party dependency tree is rustls throughout, and
//! `openssl` appears in the lock only through the legacy `websocket 0.27.1`
//! chain. Taking `native-tls` here would add a system library to every build
//! and every container image for one subsystem.
//!
//! # Why a failure is classified before it is described
//!
//! FR-014 says a failed test must name what to fix and must never print a
//! password. A `lettre` error's own `Display` is a debugging string, not that
//! sentence, and stringifying it is how a credential ends up on a screen. So
//! [`classify`] reduces an error to one of a closed set of *situations*, and
//! [`failure_for`] turns a situation into prose — which means the FR-014 test
//! can enumerate every situation rather than every error a server might send.
//!
//! [`scrub`] is the belt to that braces: whatever prose is produced, and
//! whatever verbatim refusal text a receiving server sent back, every
//! configured value is removed from it before it leaves this module. A rule
//! enforced by remembering is a rule that holds until the next contributor.

use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::Mailbox};

use super::{Availability, DeliveryFailure, MailTransport, OutgoingMessage};
use crate::settings::resolver::Settings;

/// How the connection is secured. The three the declaration allows, and no
/// fourth — `settings::registry` validates the same list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Security {
    None,
    StartTls,
    Implicit,
}

/// The `mail.*` settings, resolved into the shape a connection needs.
///
/// Deliberately not `Debug`: a struct holding a password that derives `Debug`
/// is a password one `{:?}` away from a log line, and there is no diagnostic
/// this would help with that `Availability` does not already answer.
#[derive(Clone)]
pub struct MailConfig {
    pub host: String,
    pub port: u16,
    pub security: Security,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from_address: String,
    pub from_name: Option<String>,
}

impl MailConfig {
    /// Build from the settings as they resolve now.
    ///
    /// `Err` is the **key** at fault, never the value that was wrong — the
    /// caller turns it into a gap naming a setting, exactly as readiness does.
    pub fn from_settings(settings: &Settings) -> Result<MailConfig, &'static str> {
        let host = settings.value("mail.host").ok_or("mail.host")?.to_string();
        let port: u16 = settings
            .value("mail.port")
            .ok_or("mail.port")?
            .trim()
            .parse()
            .map_err(|_| "mail.port")?;
        let security = match settings
            .value("mail.security")
            .ok_or("mail.security")?
            .trim()
        {
            "none" => Security::None,
            "starttls" => Security::StartTls,
            "implicit" => Security::Implicit,
            _ => return Err("mail.security"),
        };
        let from_address = settings
            .value("mail.from_address")
            .ok_or("mail.from_address")?
            .to_string();

        Ok(MailConfig {
            host,
            port,
            security,
            username: settings.value("mail.username").map(str::to_string),
            password: settings.value("mail.password").map(str::to_string),
            from_address,
            from_name: settings.value("mail.from_name").map(str::to_string),
        })
    }

    /// Every configured value that must never appear in operator-facing prose.
    ///
    /// The host is on this list as well as the credentials: contracts/mail.md
    /// rule 3 is "names what to fix and nothing else", and a refusal that
    /// prints the host is a refusal an operator can paste into a chat room.
    fn secrets(&self) -> Vec<String> {
        let mut values = vec![self.host.clone()];
        values.extend(self.username.clone());
        values.extend(self.password.clone());
        values.retain(|v| !v.trim().is_empty());
        values
    }
}

/// The situations a send can fail in. A closed set, so the FR-014 test can be
/// exhaustive rather than representative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureClass {
    /// The address in `mail.from_address`, or the recipient's, is not one a
    /// message can be built with.
    Address,
    /// The connection could not be made at all.
    Connection,
    /// It was made, and nothing came back in time.
    Timeout,
    /// The TLS handshake failed — nearly always `mail.security` disagreeing
    /// with `mail.port`.
    Tls,
    /// The server refused the credentials.
    Authentication,
    /// The receiver refused the message and will refuse it again.
    Rejected { code: String, text: String },
    /// The receiver refused the message for now.
    TemporarilyRejected { code: String, text: String },
    /// Something `lettre` reported that none of the above describes.
    Other,
}

/// The status codes an SMTP server answers an authentication problem with.
/// RFC 4954 § 6: 530 (auth required), 534/535 (mechanism/credentials refused),
/// 538 (encryption required for the mechanism).
const AUTHENTICATION_CODES: [&str; 4] = ["530", "534", "535", "538"];

/// Reduce a `lettre` error to one of the situations above.
pub fn classify(error: &lettre::transport::smtp::Error) -> FailureClass {
    use std::error::Error as _;

    if error.is_timeout() {
        return FailureClass::Timeout;
    }
    if error.is_tls() {
        return FailureClass::Tls;
    }

    let text = error
        .source()
        .map(|source| source.to_string())
        .unwrap_or_default();

    match error.status() {
        Some(code) => {
            let code = code.to_string();
            if AUTHENTICATION_CODES.contains(&code.as_str()) {
                FailureClass::Authentication
            } else if error.is_permanent() {
                FailureClass::Rejected { code, text }
            } else {
                FailureClass::TemporarilyRejected { code, text }
            }
        }
        None if error.is_client() || error.is_response() => FailureClass::Other,
        None => FailureClass::Connection,
    }
}

/// Turn a situation into the sentence an operator reads.
///
/// Every branch names settings and nothing else, except the two that quote the
/// receiving server's own refusal — which contracts/mail.md asks for verbatim,
/// and which [`scrub`] still passes over on the way out.
pub fn failure_for(class: FailureClass, config: &MailConfig) -> DeliveryFailure {
    let failure = match class {
        FailureClass::Address => DeliveryFailure::permanent(
            "The message could not be addressed. Check `mail.from_address`, and \
             that the recipient's address is a real one.",
        ),
        FailureClass::Connection => DeliveryFailure::transient(
            "This instance could not reach the mail server. Check `mail.host` \
             and `mail.port`, and that this instance is allowed to open an \
             outbound connection to it.",
        ),
        FailureClass::Timeout => DeliveryFailure::transient(
            "The mail server accepted a connection and then did not answer in \
             time. Check `mail.host` and `mail.port`.",
        ),
        FailureClass::Tls => DeliveryFailure::transient(
            "The secure connection to the mail server could not be \
             established. Check that `mail.security` matches `mail.port` — \
             `starttls` is usually 587 and `implicit` is usually 465 — and \
             that the server's certificate is one this instance trusts.",
        ),
        FailureClass::Authentication => DeliveryFailure::permanent(
            "Authentication was refused by the mail server. Check \
             `mail.username` and `mail.password`. If the account uses \
             two-factor authentication, it probably needs an application \
             password rather than the account's own.",
        ),
        FailureClass::Rejected { code, text } => DeliveryFailure::permanent(format!(
            "The mail server refused the message and will refuse it again \
             ({code}). It said: {text}"
        )),
        FailureClass::TemporarilyRejected { code, text } => DeliveryFailure::transient(format!(
            "The mail server refused the message for now ({code}). It said: {text}"
        )),
        FailureClass::Other => DeliveryFailure::transient(
            "The mail server did not complete the exchange. Check `mail.host`, \
             `mail.port` and `mail.security`.",
        ),
    };

    DeliveryFailure {
        reason: scrub(&failure.reason, config),
        retryable: failure.retryable,
    }
}

/// Remove every configured value from operator-facing prose.
///
/// The last thing between a receiving server's verbatim text and a screen. It
/// replaces rather than truncates, so the sentence still reads and the reader
/// can see that something was withheld — and it says which setting to look at,
/// which is the useful half of what was removed.
pub fn scrub(reason: &str, config: &MailConfig) -> String {
    let mut scrubbed = reason.to_string();
    for value in config.secrets() {
        if scrubbed.contains(&value) {
            scrubbed = scrubbed.replace(&value, "[a configured value]");
        }
    }
    scrubbed
}

/// SMTP, for real.
pub struct SmtpTransport {
    config: MailConfig,
}

impl SmtpTransport {
    pub fn new(config: MailConfig) -> Self {
        SmtpTransport { config }
    }

    /// `Err` is the setting key at fault (see [`MailConfig::from_settings`]).
    pub fn from_settings(settings: &Settings) -> Result<Self, &'static str> {
        MailConfig::from_settings(settings).map(SmtpTransport::new)
    }

    fn relay(&self) -> Result<AsyncSmtpTransport<Tokio1Executor>, DeliveryFailure> {
        let builder = match self.config.security {
            Security::Implicit => AsyncSmtpTransport::<Tokio1Executor>::relay(&self.config.host),
            Security::StartTls => {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.host)
            }
            // `builder_dangerous` is the honest name for what `mail.security =
            // none` asks for, and an operator who chose it chose it. Mailpit
            // and an in-network relay are the reasons it exists.
            Security::None => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(
                &self.config.host,
            )),
        }
        .map_err(|e| failure_for(classify(&e), &self.config))?;

        let mut builder = builder.port(self.config.port);
        if let (Some(username), Some(password)) = (&self.config.username, &self.config.password) {
            builder = builder.credentials(Credentials::new(username.clone(), password.clone()));
        }
        Ok(builder.build())
    }

    fn envelope(&self, message: &OutgoingMessage) -> Result<Message, DeliveryFailure> {
        let from: Mailbox = match &self.config.from_name {
            Some(name) => format!("{name} <{}>", self.config.from_address).parse(),
            None => self.config.from_address.parse(),
        }
        .map_err(|_| failure_for(FailureClass::Address, &self.config))?;
        let to: Mailbox = message
            .to
            .parse()
            .map_err(|_| failure_for(FailureClass::Address, &self.config))?;

        Message::builder()
            .from(from)
            .to(to)
            .subject(message.subject.clone())
            .body(message.body_text.clone())
            .map_err(|_| failure_for(FailureClass::Address, &self.config))
    }
}

#[async_trait::async_trait]
impl MailTransport for SmtpTransport {
    async fn send(&self, message: &OutgoingMessage) -> Result<(), DeliveryFailure> {
        let relay = self.relay()?;
        let envelope = self.envelope(message)?;
        relay
            .send(envelope)
            .await
            .map(|_| ())
            .map_err(|e| failure_for(classify(&e), &self.config))
    }

    fn availability(&self) -> Availability {
        // A transport that exists at all was built from a complete set of
        // settings; whether the server on the other end will have us is a
        // question only sending answers, and `sendTestMail` is how an operator
        // asks it (FR-013).
        Availability::Ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWORD: &str = "correct-horse-battery-staple";
    const USERNAME: &str = "postmaster@operator.invalid";
    const HOST: &str = "smtp.operator.invalid";

    fn config() -> MailConfig {
        MailConfig {
            host: HOST.to_string(),
            port: 587,
            security: Security::StartTls,
            username: Some(USERNAME.to_string()),
            password: Some(PASSWORD.to_string()),
            from_address: "no-reply@operator.invalid".to_string(),
            from_name: Some("ThunderForge".to_string()),
        }
    }

    fn every_class() -> Vec<FailureClass> {
        // Both response classes are given text that contains every configured
        // value, because that is the realistic worst case: a receiving server
        // echoing the account it refused back at us.
        let echoed = format!("5.7.8 authentication failed for {USERNAME} on {HOST} ({PASSWORD})");
        vec![
            FailureClass::Address,
            FailureClass::Connection,
            FailureClass::Timeout,
            FailureClass::Tls,
            FailureClass::Authentication,
            FailureClass::Rejected {
                code: "550".to_string(),
                text: echoed.clone(),
            },
            FailureClass::TemporarilyRejected {
                code: "451".to_string(),
                text: echoed,
            },
            FailureClass::Other,
        ]
    }

    /// FR-014 and SC-004, mechanically, across **every** failure situation
    /// rather than the ones somebody remembered — including the two that quote
    /// the receiving server verbatim.
    #[test]
    fn no_failure_reason_contains_a_configured_value() {
        let config = config();
        for class in every_class() {
            let failure = failure_for(class.clone(), &config);
            for leaked in [PASSWORD, USERNAME, HOST] {
                assert!(
                    !failure.reason.contains(leaked),
                    "{class:?} leaked a configured value: {}",
                    failure.reason
                );
            }
            assert!(
                !failure.reason.contains(&PASSWORD.len().to_string()),
                "{class:?} leaked the password's length"
            );
            for fragment in ["correct", "horse", "staple"] {
                assert!(
                    !failure.reason.contains(fragment),
                    "{class:?} leaked a fragment of the password"
                );
            }
        }
    }

    /// A reason that says nothing is as useless as one that says too much.
    /// Every situation names at least one setting an operator can go and look
    /// at.
    #[test]
    fn every_failure_reason_names_a_setting_to_look_at() {
        let config = config();
        for class in every_class() {
            let failure = failure_for(class.clone(), &config);
            assert!(
                failure.reason.contains("`mail.") || failure.reason.contains("It said:"),
                "{class:?} said nothing actionable: {}",
                failure.reason
            );
        }
    }

    /// The retryability of each situation is what decides `queued` versus
    /// `failed`, so it is asserted rather than assumed.
    #[test]
    fn a_permanent_refusal_is_not_retried_and_a_transient_one_is() {
        let config = config();
        assert!(!failure_for(FailureClass::Authentication, &config).retryable);
        assert!(!failure_for(FailureClass::Address, &config).retryable);
        assert!(
            !failure_for(
                FailureClass::Rejected {
                    code: "550".into(),
                    text: "no such user".into()
                },
                &config
            )
            .retryable
        );
        assert!(failure_for(FailureClass::Connection, &config).retryable);
        assert!(failure_for(FailureClass::Timeout, &config).retryable);
        assert!(
            failure_for(
                FailureClass::TemporarilyRejected {
                    code: "451".into(),
                    text: "try later".into()
                },
                &config
            )
            .retryable
        );
    }

    /// The receiver's own words survive, minus the values. FR-016's edge case
    /// — "connects, and the receiver rejects the message" — is only useful if
    /// the operator can read what the receiver said.
    #[test]
    fn a_receivers_refusal_is_quoted_with_its_code() {
        let failure = failure_for(
            FailureClass::Rejected {
                code: "550".to_string(),
                text: "5.1.1 recipient address rejected: user unknown".to_string(),
            },
            &config(),
        );
        assert!(failure.reason.contains("550"), "{}", failure.reason);
        assert!(
            failure.reason.contains("user unknown"),
            "{}",
            failure.reason
        );
    }

    #[test]
    fn scrubbing_replaces_rather_than_deletes_so_the_sentence_still_reads() {
        let scrubbed = scrub(&format!("the server at {HOST} said no"), &config());
        assert_eq!(scrubbed, "the server at [a configured value] said no");
    }

    /// A config with no credentials scrubs nothing but the host, and does not
    /// mistake `None` for an empty string worth replacing everywhere.
    #[test]
    fn an_unauthenticated_config_scrubs_only_what_it_has() {
        let mut config = config();
        config.username = None;
        config.password = None;
        assert_eq!(scrub("a plain sentence", &config), "a plain sentence");
    }

    /// An unset setting is reported as its own key, not as a generic failure.
    /// The caller turns that key into a readiness gap, so a vague error here
    /// becomes a vague instruction there.
    #[test]
    fn an_incomplete_configuration_names_the_key_at_fault() {
        let settings = Settings::default();
        assert_eq!(
            MailConfig::from_settings(&settings).err(),
            Some("mail.host")
        );
    }
}
