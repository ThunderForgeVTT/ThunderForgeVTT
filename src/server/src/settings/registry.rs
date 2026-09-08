//! The declaration list — every setting this instance has, in code.
//!
//! # Why a list in code and not rows in a table
//!
//! A setting that can be added by inserting a row is a setting that can be
//! added without an environment variable name, without a redaction rule and
//! without saying what it enables. That is FR-012 failing quietly. Here, a
//! setting exists because a declaration exists, and a declaration cannot omit
//! any of those things because the struct has no default for them.
//!
//! The consequence is the property this feature is for: **a row in
//! `instance_settings` whose key is not declared does not resolve.** It is not
//! deleted — an operator who downgraded and upgraded again keeps their values —
//! it is simply inert, and readiness reports it as unrecognised.
//!
//! # What is declared here and what only passes through
//!
//! Three backing stores, one interface (research.md § R6). `Row` is
//! `instance_settings` and is everything new. `ManifestFile` is the six keys
//! `admin::editable_manifest_keys` has always owned, still read from and
//! written to `manifest.json` by the existing path — presented here, not
//! migrated. `AccessPolicy` is spec 035's own singleton and its own audit
//! trail, read here so it appears in one list and obeys FR-009/FR-010, written
//! only through `setInstanceAccessPolicy`.

/// What a setting enables. A gap in readiness is always a gap *in* one of
/// these, so an operator reads "what can this instance not do" rather than
/// "which keys are empty".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    /// The instance can say who runs it — which the legal pages and the
    /// support surfaces both name.
    IdentifyOperator,
    SendMail,
    PublishBeyondWorld,
    PublishTerms,
    SyncLore,
    Feedback,
}

impl Capability {
    pub fn key(self) -> &'static str {
        match self {
            Capability::IdentifyOperator => "identify_operator",
            Capability::SendMail => "send_mail",
            Capability::PublishBeyondWorld => "publish_beyond_world",
            Capability::PublishTerms => "publish_terms",
            Capability::SyncLore => "sync_lore",
            Capability::Feedback => "feedback",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Capability::IdentifyOperator => "Say who operates this instance",
            Capability::SendMail => "Send mail",
            Capability::PublishBeyondWorld => "Publish content beyond a world",
            Capability::PublishTerms => "Publish complete terms of service",
            Capability::SyncLore => "Synchronise lore with a repository",
            Capability::Feedback => "Raise feedback on the project's repository",
        }
    }

    /// Every capability, in the order an operator should read them. Readiness
    /// reports all of them every time, including the available ones: FR-025
    /// scenario 3 wants a positive assertion, and an empty list reads as "we
    /// did not check".
    pub fn all() -> &'static [Capability] {
        &[
            Capability::IdentifyOperator,
            Capability::SendMail,
            Capability::PublishBeyondWorld,
            Capability::PublishTerms,
            Capability::SyncLore,
            Capability::Feedback,
        ]
    }
}

/// Where a declared setting's stored value actually lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backing {
    /// `instance_settings`. Everything this feature introduces.
    Row,
    /// One of `admin::editable_manifest_keys()`, still in `manifest.json`.
    ManifestFile(&'static str),
    /// Spec 035's `instance_access_settings`, read here and written there.
    AccessPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    Optional,
    /// Setup does not complete while this is unset (FR-002, FR-003).
    RequiredAtSetup,
    /// Unset is a readiness gap and a refusal at the point of use. It is
    /// **never** a reason the server fails to start (FR-028, SC-009).
    RequiredFor(Capability),
}

/// The shape of a value, which decides how it is validated and how it is
/// rendered. `Secret` is not a kind — secrecy is orthogonal to shape and lives
/// in its own field, so a secret cannot be declared by choosing the wrong kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    Email,
    Url,
    Port,
    Bool,
    Enum(&'static [&'static str]),
    /// Free prose an operator supplies for a legal document. Long, multi-line,
    /// and the one kind with no sensible environment form.
    Prose,
}

/// One validation rule. A closed list rather than a heuristic: research.md
/// § R15 — a cleverer placeholder detector rejects somebody's real name, and a
/// reserved-TLD rule is both exactly right and explainable in the refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validator {
    NonEmptyAfterTrim,
    EmailSyntax,
    /// RFC 2606 / 6761 reserved TLDs — `.local`, `.example`, `.invalid`,
    /// `.test`. An address there can never receive a copyright notice.
    NoReservedTld,
    /// The addresses and names this product itself ships as placeholders. The
    /// single most likely way to publish a page naming nobody is to accept the
    /// default unchanged, so the defaults are refused by name.
    NotAShippedPlaceholder,
    PortRange,
    BoolLike,
    OneOf(&'static [&'static str]),
}

/// One setting. Every field is required, which is the point: a new setting
/// cannot forget its environment name, its redaction or what it enables.
#[derive(Debug, Clone, Copy)]
pub struct SettingDeclaration {
    pub key: &'static str,
    pub kind: Kind,
    pub backing: Backing,
    /// The variable the environment sets this by. `None` only for `Prose`,
    /// which has no sensible environment form — every other declaration has
    /// one, because the spec's edge case "an operator wants no configuration
    /// UI at all" is only true if it is literally true (FR-010).
    pub env_var: Option<&'static str>,
    /// Other variables that also set this value, in order. Exists for exactly
    /// one shape: a PEM private key, which `repo_host` accepts as a file path,
    /// as base64 or inline. Reporting `fixed_by` as whichever one is actually
    /// set is the difference between a true answer and a plausible one.
    pub env_aliases: &'static [&'static str],
    pub requirement: Requirement,
    /// Decides encryption at rest **and** every rendering, everywhere. A
    /// declaration with this set has exactly two renderings in the product:
    /// set, and not set.
    pub secret: bool,
    pub default: Option<&'static str>,
    pub validators: &'static [Validator],
    /// What this setting contributes to. `RequiredFor(c)` implies `Some(c)`;
    /// an optional setting may still belong to one (an SMTP username is not
    /// required, but it is part of sending mail).
    pub capability: Option<Capability>,
    /// What an operator should do, in a sentence. Readiness renders this
    /// verbatim, so it names a variable and never a value.
    pub what_to_set: &'static str,
    /// What is limited while this is unset.
    pub what_is_limited: &'static str,
    /// Which group an editing surface should file this under, so a person
    /// setting "who a copyright notice is served on" is not doing it in a list
    /// of thirty keys.
    pub group: &'static str,
    /// The version that introduced it — FR-028's "an upgrade must ask, not
    /// break" needs to be able to say when the asking started.
    pub since: &'static str,
}

impl SettingDeclaration {
    /// The variable that has fixed this value, if any. Checked in declaration
    /// order, primary first.
    pub fn env_name_in_use(&self) -> Option<&'static str> {
        self.env_var
            .into_iter()
            .chain(self.env_aliases.iter().copied())
            .find(|name| read_env(name).is_some())
    }

    pub fn is_required_at_setup(&self) -> bool {
        matches!(self.requirement, Requirement::RequiredAtSetup)
    }

    pub fn required_for(&self) -> Option<Capability> {
        match self.requirement {
            Requirement::RequiredFor(c) => Some(c),
            _ => None,
        }
    }
}

/// A trimmed, non-empty environment variable, or nothing.
///
/// An exported-but-empty variable is how a container platform expresses "I did
/// not set this", and reading it as a configured empty string is how an
/// instance ends up with a blank operator name it cannot edit.
pub(crate) fn read_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Keys the instance keeps for its own bookkeeping.
///
/// A row under this prefix is not a setting: it does not resolve, it is not
/// editable, and it is not reported as unrecognised. There is exactly one
/// today — `readiness`'s record of the sources it saw at the last boot — and
/// the prefix exists so that adding a second does not look like an operator's
/// stale configuration.
pub const RESERVED_PREFIX: &str = "system.";

const EMAIL: &[Validator] = &[
    Validator::NonEmptyAfterTrim,
    Validator::EmailSyntax,
    Validator::NoReservedTld,
    Validator::NotAShippedPlaceholder,
];
const NAME: &[Validator] = &[
    Validator::NonEmptyAfterTrim,
    Validator::NotAShippedPlaceholder,
];
const TEXT: &[Validator] = &[Validator::NonEmptyAfterTrim];
const NONE: &[Validator] = &[];

const MAIL_SECURITY: &[&str] = &["none", "starttls", "implicit"];
const ACCESS_POLICIES: &[&str] = &["open", "invite_only", "closed"];

/// Every setting this instance has.
///
/// Order is the order an operator reads them in, and it is the order the admin
/// surface renders: who runs this, who a notice is served on, how people get
/// help, then the subsystems.
pub fn declarations() -> &'static [SettingDeclaration] {
    &DECLARATIONS
}

/// The declaration for one key, or nothing — which is the whole enforcement
/// mechanism. Nothing resolves, validates or writes without going through
/// here.
pub fn declaration(key: &str) -> Option<&'static SettingDeclaration> {
    DECLARATIONS.iter().find(|d| d.key == key)
}

static DECLARATIONS: [SettingDeclaration; 33] = [
    // -- Who operates this instance -----------------------------------------
    SettingDeclaration {
        key: "operator.name",
        kind: Kind::Text,
        backing: Backing::Row,
        env_var: Some("THUNDERFORGE_OPERATOR_NAME"),
        env_aliases: &[],
        requirement: Requirement::RequiredAtSetup,
        secret: false,
        default: None,
        validators: NAME,
        capability: Some(Capability::IdentifyOperator),
        what_to_set: "The name of the person or organisation that runs this instance, as it should appear on the legal pages.",
        what_is_limited: "The terms of service and privacy policy name nobody in particular.",
        group: "Operator",
        since: "0.40",
    },
    SettingDeclaration {
        key: "operator.contact_email",
        kind: Kind::Email,
        backing: Backing::Row,
        env_var: Some("THUNDERFORGE_OPERATOR_CONTACT_EMAIL"),
        env_aliases: &[],
        requirement: Requirement::RequiredAtSetup,
        secret: false,
        default: None,
        validators: EMAIL,
        capability: Some(Capability::IdentifyOperator),
        what_to_set: "An address that reaches whoever runs this instance.",
        what_is_limited: "Nobody outside the instance can reach its operator.",
        group: "Operator",
        since: "0.40",
    },
    SettingDeclaration {
        key: "operator.jurisdiction",
        kind: Kind::Prose,
        backing: Backing::Row,
        env_var: Some("THUNDERFORGE_OPERATOR_JURISDICTION"),
        env_aliases: &[],
        requirement: Requirement::RequiredFor(Capability::PublishTerms),
        secret: false,
        default: None,
        validators: TEXT,
        capability: Some(Capability::PublishTerms),
        what_to_set: "The jurisdiction whose law the terms of service are read under.",
        what_is_limited: "The terms of service leave the governing law blank.",
        group: "Operator",
        since: "0.40",
    },
    // -- Where a copyright notice is served ---------------------------------
    //
    // Deliberately NOT `support_email`. 17 U.S.C. § 512(c)(2) treats the agent
    // a claim is served on as a different thing, with a different address
    // requirement, from "somebody will help you with the app".
    SettingDeclaration {
        key: "notice.contact_name",
        kind: Kind::Text,
        backing: Backing::Row,
        env_var: Some("THUNDERFORGE_NOTICE_CONTACT_NAME"),
        env_aliases: &[],
        requirement: Requirement::RequiredFor(Capability::PublishBeyondWorld),
        secret: false,
        default: None,
        validators: NAME,
        capability: Some(Capability::PublishBeyondWorld),
        what_to_set: "The name of the person or role a copyright notice is served on.",
        what_is_limited: "Content cannot be published beyond a world. Playing privately is unaffected.",
        group: "Copyright notices",
        since: "0.40",
    },
    SettingDeclaration {
        key: "notice.contact_email",
        kind: Kind::Email,
        backing: Backing::Row,
        env_var: Some("THUNDERFORGE_NOTICE_CONTACT_EMAIL"),
        env_aliases: &[],
        requirement: Requirement::RequiredFor(Capability::PublishBeyondWorld),
        secret: false,
        default: None,
        validators: EMAIL,
        capability: Some(Capability::PublishBeyondWorld),
        what_to_set: "An address a copyright notice can be served on, reachable without an account.",
        what_is_limited: "Content cannot be published beyond a world. Playing privately is unaffected.",
        group: "Copyright notices",
        since: "0.40",
    },
    SettingDeclaration {
        key: "notice.contact_postal_address",
        kind: Kind::Text,
        backing: Backing::Row,
        env_var: Some("THUNDERFORGE_NOTICE_CONTACT_ADDRESS"),
        env_aliases: &[],
        requirement: Requirement::RequiredFor(Capability::PublishBeyondWorld),
        secret: false,
        default: None,
        validators: NAME,
        capability: Some(Capability::PublishBeyondWorld),
        what_to_set: "A postal address for the notice contact, which the designation is required to carry.",
        what_is_limited: "Content cannot be published beyond a world. Playing privately is unaffected.",
        group: "Copyright notices",
        since: "0.40",
    },
    // -- Operator prose for the legal pages ---------------------------------
    //
    // The three settings with no environment form. A multi-paragraph clause is
    // not something an environment variable holds, and pretending otherwise
    // would produce a variable nobody could use.
    SettingDeclaration {
        key: "legal.terms_change_notice",
        kind: Kind::Prose,
        backing: Backing::Row,
        env_var: None,
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: false,
        default: None,
        validators: NONE,
        capability: Some(Capability::PublishTerms),
        what_to_set: "How this instance tells people the terms have changed.",
        what_is_limited: "The terms of service keep the shipped placeholder for that clause.",
        group: "Legal prose",
        since: "0.40",
    },
    SettingDeclaration {
        key: "legal.community_addendum",
        kind: Kind::Prose,
        backing: Backing::Row,
        env_var: None,
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: false,
        default: None,
        validators: NONE,
        capability: Some(Capability::PublishTerms),
        what_to_set: "Any additional rules this community adds to the shipped terms.",
        what_is_limited: "The terms of service carry no community-specific rules.",
        group: "Legal prose",
        since: "0.40",
    },
    SettingDeclaration {
        key: "legal.minimum_age_statement",
        kind: Kind::Prose,
        backing: Backing::Row,
        env_var: None,
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: false,
        default: None,
        validators: NONE,
        capability: Some(Capability::PublishTerms),
        what_to_set: "The minimum age this instance admits, in the operator's own words.",
        what_is_limited: "The terms of service keep the shipped placeholder for the age statement.",
        group: "Legal prose",
        since: "0.40",
    },
    // -- Mail ---------------------------------------------------------------
    //
    // Declared here, delivered elsewhere: the transport, the outbox and the
    // sender are spec 040's US4. A declaration with nothing behind it yet is
    // still worth having, because it is what lets readiness say "this instance
    // cannot send mail, and here is what to set" rather than saying nothing.
    SettingDeclaration {
        key: "mail.enabled",
        kind: Kind::Bool,
        backing: Backing::Row,
        env_var: Some("THUNDERFORGE_SMTP_ENABLED"),
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: false,
        default: Some("false"),
        validators: &[Validator::BoolLike],
        capability: Some(Capability::SendMail),
        what_to_set: "Turn mail on once the server details below are right.",
        what_is_limited: "Nothing is sent; messages wait rather than being discarded.",
        group: "Mail",
        since: "0.40",
    },
    SettingDeclaration {
        key: "mail.host",
        kind: Kind::Text,
        backing: Backing::Row,
        env_var: Some("THUNDERFORGE_SMTP_HOST"),
        env_aliases: &[],
        requirement: Requirement::RequiredFor(Capability::SendMail),
        secret: false,
        default: None,
        validators: TEXT,
        capability: Some(Capability::SendMail),
        what_to_set: "The hostname of the SMTP server this instance sends through.",
        what_is_limited: "The instance cannot send mail. Everything else works.",
        group: "Mail",
        since: "0.40",
    },
    SettingDeclaration {
        key: "mail.port",
        kind: Kind::Port,
        backing: Backing::Row,
        env_var: Some("THUNDERFORGE_SMTP_PORT"),
        env_aliases: &[],
        requirement: Requirement::RequiredFor(Capability::SendMail),
        secret: false,
        default: Some("587"),
        validators: &[Validator::PortRange],
        capability: Some(Capability::SendMail),
        what_to_set: "The port the SMTP server listens on — 587 for STARTTLS, 465 for implicit TLS.",
        what_is_limited: "The instance cannot send mail. Everything else works.",
        group: "Mail",
        since: "0.40",
    },
    SettingDeclaration {
        key: "mail.security",
        kind: Kind::Enum(MAIL_SECURITY),
        backing: Backing::Row,
        env_var: Some("THUNDERFORGE_SMTP_SECURITY"),
        env_aliases: &[],
        requirement: Requirement::RequiredFor(Capability::SendMail),
        secret: false,
        default: Some("starttls"),
        validators: &[Validator::OneOf(MAIL_SECURITY)],
        capability: Some(Capability::SendMail),
        what_to_set: "How the connection is secured: none, starttls or implicit.",
        what_is_limited: "The instance cannot send mail. Everything else works.",
        group: "Mail",
        since: "0.40",
    },
    SettingDeclaration {
        key: "mail.username",
        kind: Kind::Text,
        backing: Backing::Row,
        env_var: Some("THUNDERFORGE_SMTP_USERNAME"),
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: false,
        default: None,
        validators: TEXT,
        capability: Some(Capability::SendMail),
        what_to_set: "The account the SMTP server authenticates, when it asks for one.",
        what_is_limited: "Nothing, unless the server requires authentication.",
        group: "Mail",
        since: "0.40",
    },
    SettingDeclaration {
        key: "mail.password",
        kind: Kind::Text,
        backing: Backing::Row,
        env_var: Some("THUNDERFORGE_SMTP_PASSWORD"),
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: true,
        default: None,
        validators: &[Validator::NonEmptyAfterTrim],
        capability: Some(Capability::SendMail),
        what_to_set: "The password for that account. It is stored encrypted and never displayed again.",
        what_is_limited: "Nothing, unless the server requires authentication.",
        group: "Mail",
        since: "0.40",
    },
    SettingDeclaration {
        key: "mail.from_address",
        kind: Kind::Email,
        backing: Backing::Row,
        env_var: Some("THUNDERFORGE_SMTP_FROM_ADDRESS"),
        env_aliases: &[],
        requirement: Requirement::RequiredFor(Capability::SendMail),
        secret: false,
        default: None,
        validators: EMAIL,
        capability: Some(Capability::SendMail),
        what_to_set: "The address mail from this instance is sent from.",
        what_is_limited: "The instance cannot send mail. Everything else works.",
        group: "Mail",
        since: "0.40",
    },
    SettingDeclaration {
        key: "mail.from_name",
        kind: Kind::Text,
        backing: Backing::Row,
        env_var: Some("THUNDERFORGE_SMTP_FROM_NAME"),
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: false,
        default: None,
        validators: TEXT,
        capability: Some(Capability::SendMail),
        what_to_set: "The name that appears beside the sending address.",
        what_is_limited: "Mail is sent from a bare address with no display name.",
        group: "Mail",
        since: "0.40",
    },
    // -- GitHub applications ------------------------------------------------
    //
    // Declared here so they resolve by the one rule and appear in one list.
    // Which application a subsystem actually uses, and what a partially
    // specified one falls back to, is spec 040's US5 and is decided in its own
    // module — not here, and not by this file's ordering.
    SettingDeclaration {
        key: "github_app.global.client_id",
        kind: Kind::Text,
        backing: Backing::Row,
        env_var: Some("GLOBAL_GITHUB_APP_CLIENT_ID"),
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: false,
        default: None,
        validators: TEXT,
        capability: None,
        what_to_set: "The client ID of a GitHub application every subsystem may fall back to.",
        what_is_limited: "Each subsystem needs its own application.",
        group: "GitHub applications",
        since: "0.40",
    },
    SettingDeclaration {
        key: "github_app.global.slug",
        kind: Kind::Text,
        backing: Backing::Row,
        env_var: Some("GLOBAL_GITHUB_APP_SLUG"),
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: false,
        default: None,
        validators: TEXT,
        capability: None,
        what_to_set: "That application's URL slug, which the install link is built from.",
        what_is_limited: "Each subsystem needs its own application.",
        group: "GitHub applications",
        since: "0.40",
    },
    SettingDeclaration {
        key: "github_app.global.private_key",
        kind: Kind::Text,
        backing: Backing::Row,
        env_var: Some("GLOBAL_GITHUB_APP_PRIVATE_KEY"),
        env_aliases: &[
            "GLOBAL_GITHUB_APP_PRIVATE_KEY_FILE",
            "GLOBAL_GITHUB_APP_PRIVATE_KEY_BASE64",
        ],
        requirement: Requirement::Optional,
        secret: true,
        default: None,
        validators: &[Validator::NonEmptyAfterTrim],
        capability: None,
        what_to_set: "That application's PEM private key. A path or base64 form may be given by the environment instead.",
        what_is_limited: "Each subsystem needs its own application.",
        group: "GitHub applications",
        since: "0.40",
    },
    SettingDeclaration {
        key: "github_app.sync.client_id",
        kind: Kind::Text,
        backing: Backing::Row,
        env_var: Some(crate::repo_host::APP_ID_ENV),
        env_aliases: &[],
        requirement: Requirement::RequiredFor(Capability::SyncLore),
        secret: false,
        default: None,
        validators: TEXT,
        capability: Some(Capability::SyncLore),
        what_to_set: "The client ID of the GitHub application lore synchronisation acts as.",
        what_is_limited: "A world cannot be connected to a repository.",
        group: "GitHub applications",
        since: "0.40",
    },
    SettingDeclaration {
        key: "github_app.sync.slug",
        kind: Kind::Text,
        backing: Backing::Row,
        env_var: Some(crate::repo_host::APP_SLUG_ENV),
        env_aliases: &[],
        requirement: Requirement::RequiredFor(Capability::SyncLore),
        secret: false,
        default: None,
        validators: TEXT,
        capability: Some(Capability::SyncLore),
        what_to_set: "That application's URL slug. Not interchangeable with the client ID.",
        what_is_limited: "A world cannot be connected to a repository.",
        group: "GitHub applications",
        since: "0.40",
    },
    SettingDeclaration {
        key: "github_app.sync.private_key",
        kind: Kind::Text,
        backing: Backing::Row,
        env_var: Some(crate::repo_host::APP_PRIVATE_KEY_ENV),
        env_aliases: &[
            crate::repo_host::APP_PRIVATE_KEY_FILE_ENV,
            crate::repo_host::APP_PRIVATE_KEY_BASE64_ENV,
        ],
        requirement: Requirement::RequiredFor(Capability::SyncLore),
        secret: true,
        default: None,
        validators: &[Validator::NonEmptyAfterTrim],
        capability: Some(Capability::SyncLore),
        what_to_set: "That application's PEM private key. The environment may give a path or the base64 form instead.",
        what_is_limited: "A world cannot be connected to a repository.",
        group: "GitHub applications",
        since: "0.40",
    },
    SettingDeclaration {
        key: "github_app.feedback.client_id",
        kind: Kind::Text,
        backing: Backing::Row,
        env_var: Some("FEEDBACK_GITHUB_APP_CLIENT_ID"),
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: false,
        default: None,
        validators: TEXT,
        capability: Some(Capability::Feedback),
        what_to_set: "The client ID of the GitHub application feedback is raised as.",
        what_is_limited: "Feedback cannot be raised on the project's repository.",
        group: "GitHub applications",
        since: "0.40",
    },
    SettingDeclaration {
        key: "github_app.feedback.slug",
        kind: Kind::Text,
        backing: Backing::Row,
        env_var: Some("FEEDBACK_GITHUB_APP_SLUG"),
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: false,
        default: None,
        validators: TEXT,
        capability: Some(Capability::Feedback),
        what_to_set: "That application's URL slug.",
        what_is_limited: "Feedback cannot be raised on the project's repository.",
        group: "GitHub applications",
        since: "0.40",
    },
    SettingDeclaration {
        key: "github_app.feedback.private_key",
        kind: Kind::Text,
        backing: Backing::Row,
        env_var: Some("FEEDBACK_GITHUB_APP_PRIVATE_KEY"),
        env_aliases: &[
            "FEEDBACK_GITHUB_APP_PRIVATE_KEY_FILE",
            "FEEDBACK_GITHUB_APP_PRIVATE_KEY_BASE64",
        ],
        requirement: Requirement::Optional,
        secret: true,
        default: None,
        validators: &[Validator::NonEmptyAfterTrim],
        capability: Some(Capability::Feedback),
        what_to_set: "That application's PEM private key.",
        what_is_limited: "Feedback cannot be raised on the project's repository.",
        group: "GitHub applications",
        since: "0.40",
    },
    // -- The realm manifest, presented rather than moved ---------------------
    //
    // Still `manifest.json`, still seeded from `config/realm-defaults.json`,
    // still written through `admin::update_manifest_key`. What changes is that
    // they resolve by the same rule and report a source (research.md § R6).
    SettingDeclaration {
        key: "realm_name",
        kind: Kind::Text,
        backing: Backing::ManifestFile("realm_name"),
        env_var: Some("THUNDERFORGE_REALM_NAME"),
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: false,
        default: None,
        validators: TEXT,
        capability: None,
        what_to_set: "What this instance calls itself.",
        what_is_limited: "The instance is named as it shipped.",
        group: "Realm",
        since: "0.40",
    },
    SettingDeclaration {
        key: "support_email",
        kind: Kind::Email,
        backing: Backing::ManifestFile("support_email"),
        env_var: Some("THUNDERFORGE_SUPPORT_EMAIL"),
        env_aliases: &[],
        requirement: Requirement::RequiredAtSetup,
        secret: false,
        default: None,
        validators: EMAIL,
        capability: Some(Capability::IdentifyOperator),
        what_to_set: "An address people can reach for help with this instance.",
        what_is_limited: "Nobody using this instance has anywhere to write for help.",
        group: "Support",
        since: "0.40",
    },
    SettingDeclaration {
        key: "welcome_message",
        kind: Kind::Text,
        backing: Backing::ManifestFile("welcome_message"),
        env_var: Some("THUNDERFORGE_WELCOME_MESSAGE"),
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: false,
        default: None,
        validators: TEXT,
        capability: None,
        what_to_set: "What a new arrival is greeted with.",
        what_is_limited: "New arrivals see the greeting the instance shipped with.",
        group: "Realm",
        since: "0.40",
    },
    SettingDeclaration {
        key: "interface_pack_id",
        kind: Kind::Text,
        backing: Backing::ManifestFile("interface_pack_id"),
        env_var: Some("THUNDERFORGE_INTERFACE_PACK_ID"),
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: false,
        default: None,
        validators: TEXT,
        capability: None,
        what_to_set: "Which interface pack a new world starts with.",
        what_is_limited: "New worlds use the shipped interface pack.",
        group: "Realm",
        since: "0.40",
    },
    SettingDeclaration {
        key: "asset_pack_id",
        kind: Kind::Text,
        backing: Backing::ManifestFile("asset_pack_id"),
        env_var: Some("THUNDERFORGE_ASSET_PACK_ID"),
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: false,
        default: None,
        validators: TEXT,
        capability: None,
        what_to_set: "Which asset pack a new world starts with.",
        what_is_limited: "New worlds use the shipped asset pack.",
        group: "Realm",
        since: "0.40",
    },
    SettingDeclaration {
        key: "default_game_system_id",
        kind: Kind::Text,
        backing: Backing::ManifestFile("default_game_system_id"),
        env_var: Some("THUNDERFORGE_DEFAULT_GAME_SYSTEM_ID"),
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: false,
        default: None,
        validators: TEXT,
        capability: None,
        what_to_set: "Which game system a world starts with when nobody names one.",
        what_is_limited: "A world created without naming a system has no system at all.",
        group: "Realm",
        since: "0.40",
    },
    SettingDeclaration {
        key: "instance.access_policy",
        kind: Kind::Enum(ACCESS_POLICIES),
        backing: Backing::AccessPolicy,
        env_var: Some("THUNDERFORGE_INSTANCE_ACCESS_POLICY"),
        env_aliases: &[],
        requirement: Requirement::Optional,
        secret: false,
        default: Some("closed"),
        validators: &[Validator::OneOf(ACCESS_POLICIES)],
        capability: None,
        what_to_set: "Who may create an account: open, invite_only or closed.",
        what_is_limited: "Nothing — the instance always has a policy.",
        group: "Access",
        since: "0.40",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Two declarations sharing a key would make `declaration()` return
    /// whichever came first, and the loser would be a setting that silently
    /// stopped existing.
    #[test]
    fn every_key_is_declared_once() {
        let mut seen = HashSet::new();
        for d in declarations() {
            assert!(seen.insert(d.key), "`{}` is declared twice", d.key);
        }
    }

    /// FR-010, and the edge case behind it: an operator who wants no
    /// configuration interface at all must be able to set everything by
    /// environment. That is only true if it is literally true, so the only
    /// exemption is `Prose`, which an environment variable cannot hold.
    #[test]
    fn everything_but_prose_can_be_set_by_the_environment() {
        for d in declarations() {
            if d.kind == Kind::Prose && d.env_var.is_none() {
                continue;
            }
            assert!(
                d.env_var.is_some(),
                "`{}` has no environment variable name",
                d.key
            );
        }
    }

    /// Two settings reading the same variable would make one of them
    /// unsettable and the other surprising.
    #[test]
    fn no_two_declarations_read_the_same_variable() {
        let mut seen = HashSet::new();
        for d in declarations() {
            for name in d.env_var.into_iter().chain(d.env_aliases.iter().copied()) {
                assert!(
                    seen.insert(name),
                    "`{name}` is read by more than one declaration ({})",
                    d.key
                );
            }
        }
    }

    /// `RequiredFor(c)` and `capability: None` would produce a gap belonging
    /// to a capability the setting does not claim to be part of.
    #[test]
    fn a_required_for_declaration_names_the_capability_it_belongs_to() {
        for d in declarations() {
            if let Some(c) = d.required_for() {
                assert_eq!(
                    d.capability,
                    Some(c),
                    "`{}` is required for {} but belongs to {:?}",
                    d.key,
                    c.key(),
                    d.capability
                );
            }
        }
    }

    /// Readiness renders these verbatim, so an empty one is a gap that tells
    /// an operator nothing.
    #[test]
    fn every_declaration_says_what_to_set_and_what_is_limited() {
        for d in declarations() {
            assert!(!d.what_to_set.trim().is_empty(), "`{}`", d.key);
            assert!(!d.what_is_limited.trim().is_empty(), "`{}`", d.key);
            assert!(!d.group.trim().is_empty(), "`{}`", d.key);
        }
    }

    /// The reserved prefix is the instance's own bookkeeping. A declaration
    /// under it would be a setting an operator could edit into the machinery.
    #[test]
    fn no_declaration_uses_the_reserved_prefix() {
        for d in declarations() {
            assert!(!d.key.starts_with(RESERVED_PREFIX), "`{}`", d.key);
        }
    }

    /// A secret with a shipped default would be a shipped credential.
    #[test]
    fn no_secret_has_a_default() {
        for d in declarations() {
            assert!(!(d.secret && d.default.is_some()), "`{}`", d.key);
        }
    }
}
