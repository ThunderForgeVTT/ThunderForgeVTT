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
//!
//! Every setting this instance has, and the types that describe one.
//!
//! The declarations themselves live in [`declarations`], which is a list and
//! nothing else. They were in this file until it crossed the length the repo
//! enforces, and the split is along the only seam that was ever there: the
//! vocabulary a declaration is written in, and the declarations written in it.
//! A new setting is a new entry in that list; a new *kind* of setting is a
//! change here.

mod declarations;
use declarations::DECLARATIONS;

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
    /// The instance can store and serve an uploaded file at all \u{2014} a map, a
    /// token, a portrait, a feedback attachment.
    StoreAssets,
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
            Capability::StoreAssets => "store_assets",
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
            Capability::StoreAssets => "Store and serve uploaded files",
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
            Capability::StoreAssets,
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

/// What first-run setup does with a declaration.
///
/// Stated per declaration rather than inferred from a capability list. The
/// list came first (`CAPABILITIES_SETUP_ASKS_ABOUT`) and had the failure this
/// replaces: a new declaration joined or missed the wizard depending on which
/// capability somebody happened to tag it with, and nobody reviewing the
/// declaration could see which had happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupVisibility {
    /// Setup asks for it, on a step of its own group, every time.
    Asked,
    /// Setup asks for it only once the operator has said this instance
    /// publishes beyond a world (spec 052 US2, FR-010). A private instance is
    /// never asked, and readiness reports the capability as not applicable
    /// rather than as a gap.
    AskedWhenPublishing,
    /// Offered on the last step, with everything else an operator might have
    /// meant to set. Never blocks completion and never gets a step of its own.
    Offered,
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
    /// What first-run setup does with this declaration. A new setting cannot
    /// forget to answer it.
    pub setup: SetupVisibility,
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
