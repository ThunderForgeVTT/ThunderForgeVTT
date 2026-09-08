//! One GitHub application for everything, or one per subsystem, or a mixture —
//! and the operator knows which acts for what.
//!
//! Spec 040 US5 (FR-019 – FR-024), `contracts/github-applications.md`,
//! research.md § R10, ADR-090.
//!
//! # Why this module exists when `repo_host::scoped` already did
//!
//! Spec 037's feedback delivery landed a `registration_for(scope, &Settings)`
//! in `repo_host/scoped.rs` a few hours before this. It read the same nine
//! declarations, returned the same kind of diagnostic, and resolved **field by
//! field**: a subsystem application with a slug and no key borrowed the global
//! application's key.
//!
//! Spec 040 FR-021, its US5 acceptance scenario 4 and research.md § R10 all
//! say the opposite, and say it deliberately: *an application resolves whole*.
//! A client ID from one registration with a private key from another is not an
//! application. It is an authentication failure that reads like a bad key —
//! the same confusion `repo_host.rs` already warns about for the id/slug pair,
//! one level up.
//!
//! Two subsystems inventing two shapes for one credential is exactly what US5
//! exists to prevent, so this is a reconciliation and not a second
//! implementation: the resolution half of `repo_host::scoped` moved here
//! whole, changed from per-field to per-application, and `repo_host::scoped`
//! re-exports it so spec 037's caller and its vocabulary are untouched. The
//! effects half — the calls delivery makes against the host — stays there,
//! because talking to the host is `repo_host`'s job and choosing a credential
//! is not.
//!
//! Spec 037's own FR-029 ("when resolution draws on both global and specific
//! values…") is the sentence that assumed merging. Its pointer now names this
//! contract; see `specs/037-in-app-feedback/spec.md`.
//!
//! # Resolution order: scope outside, source inside
//!
//! ```text
//! For subsystem S, take the first COMPLETE application:
//!   1. S,      environment    <S>_GITHUB_APP_*        e.g. SYNC_GITHUB_APP_*
//!   2. S,      instance       github_app.<s>.*
//!   3. global, environment    GLOBAL_GITHUB_APP_*
//!   4. global, instance       github_app.global.*
//! ```
//!
//! FR-010 says the environment beats the instance; FR-019 says the subsystem
//! beats the global. When an operator sets a global application by environment
//! *and* a subsystem one in the admin screens, those rules point opposite
//! ways. **Specific intent wins, and the source rule applies within it** — a
//! broad `GLOBAL_GITHUB_APP_*` does not silently override the subsystem
//! application somebody just deliberately configured.
//!
//! Steps 1–2 are one `settings.get("github_app.<s>.…")` and steps 3–4 are one
//! `settings.get("github_app.global.…")`, because `settings::resolver` already
//! implements the inner axis for every declared key. The outer axis is the
//! only thing written here, which is why this file reads shorter than the rule
//! it implements.
//!
//! # Nothing here declares a variable
//!
//! `settings::registry` declares all nine keys with their environment
//! variables and their aliases. This module consumes those declarations; it
//! never reads the environment itself. A tenth variable name appearing here
//! would be a variable no readiness surface knows about.
//!
//! # The private key's form is decided by the variable, not the value
//!
//! `..._PRIVATE_KEY_FILE` means a path, `..._PRIVATE_KEY_BASE64` means base64,
//! and anything else goes through [`crate::repo_host::normalise_pem`].
//! [`Resolved::fixed_by`] names the variable that actually set it. Sniffing
//! the value is how `"not a key at all"` turned out to be valid base64 —
//! `normalise_pem` documents that at length and the mistake is not repeated
//! here.
//!
//! The *order* of those variables is the declaration's, and it is load-bearing:
//! file, then base64, then inline, the same precedence
//! `repo_host::read_private_key` has had since spec 034. It was written the
//! other way round first and a deployment setting both a `_FILE` and an inline
//! key then resolved to two different keys through the two paths — which is
//! precisely the FR-024 break this module exists to avoid.
//! `github_apps_tests::the_three_key_forms_keep_their_precedence` is that bug,
//! kept.
//!
//! # The `_FILE` form stays an environment form, deliberately
//!
//! T100 asks that the file form store a path rather than the key, so a Docker
//! secret stays a Docker secret. It does — by never being stored at all. The
//! path lives in the environment, is read at resolution time, and no part of
//! it reaches `instance_settings`; the resolver materialises nothing, which is
//! ADR-088's whole shape.
//!
//! What is *not* offered is a `private_key_file` field in the administration
//! screens. It would need a tenth declaration, and two registry invariants
//! rule that out together: every declaration must have an environment variable
//! (`everything_but_prose_can_be_set_by_the_environment`) and no two may read
//! the same one (`no_two_declarations_read_the_same_variable`). So a storable
//! file form would be a *new variable name*, not a new home for an existing
//! one — and a path typed into a database is a worse secret story than a path
//! in the environment anyway, since the instance store is the thing the file
//! form exists to stay out of. Recorded here because it is a deviation from
//! the task's wording, arrived at from its reason.
//!
//! And it is parsed **at resolution time, not at first use** (FR-022). A key
//! that is present and is not a key is the failure a presence check calls
//! "configured".

use base64::Engine as _;
use base64::engine::general_purpose;
use thunderforge_repo_host::github::GitHubApp;

use crate::settings::resolver::{Resolved, Settings};

/// Which subsystem is asking. `Global` is not a subsystem — it is the
/// fallback every other scope draws on — but it resolves like one so that an
/// operator surface can report it the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppScope {
    Global,
    Sync,
    Feedback,
}

impl AppScope {
    pub fn as_str(self) -> &'static str {
        match self {
            AppScope::Global => "global",
            AppScope::Sync => "sync",
            AppScope::Feedback => "feedback",
        }
    }

    /// Named `parse` rather than `from_str`: clippy rightly objects to an
    /// inherent `from_str` that is not `FromStr`, and this returns `Option`
    /// because an unknown scope from a GraphQL argument is a refusal, not an
    /// error type worth inventing.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "global" => Some(AppScope::Global),
            "sync" => Some(AppScope::Sync),
            "feedback" => Some(AppScope::Feedback),
            _ => None,
        }
    }

    /// Every scope an operator can configure, global first because it is the
    /// one the others fall back to.
    pub fn all() -> &'static [AppScope] {
        &[AppScope::Global, AppScope::Sync, AppScope::Feedback]
    }

    /// The scopes that are actually subsystems. `actsFor` is a list of these
    /// and never contains `global`, because "the global application acts for
    /// global" is not information.
    pub fn subsystems() -> &'static [AppScope] {
        &[AppScope::Sync, AppScope::Feedback]
    }

    /// What an operator is told this application is used for. FR-020: the
    /// global application's *scope* is an explanation, not a field.
    pub fn serves(self) -> &'static str {
        match self {
            AppScope::Global => {
                "every subsystem that talks to GitHub — lore synchronisation \
                 and feedback today, and anything added later"
            }
            AppScope::Sync => "lore synchronisation",
            AppScope::Feedback => "feedback",
        }
    }
}

/// One of the three values an application is described by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    ClientId,
    Slug,
    PrivateKey,
}

impl Field {
    pub fn as_str(self) -> &'static str {
        match self {
            Field::ClientId => "client_id",
            Field::Slug => "slug",
            Field::PrivateKey => "private_key",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "client_id" => Some(Field::ClientId),
            "slug" => Some(Field::Slug),
            "private_key" => Some(Field::PrivateKey),
            _ => None,
        }
    }

    pub fn all() -> &'static [Field] {
        &[Field::ClientId, Field::Slug, Field::PrivateKey]
    }

    /// The words a diagnostic uses for it.
    pub fn label(self) -> &'static str {
        match self {
            Field::ClientId => "client ID",
            Field::Slug => "URL slug",
            Field::PrivateKey => "private key",
        }
    }
}

/// The declared setting key for one scope and one field.
///
/// A table rather than string concatenation, because a `&'static str` is what
/// the settings registry deals in and a `format!` here would produce a key
/// that no declaration matches the moment somebody renames one. The test at
/// the bottom of this file walks every pair against `registry::declaration`.
pub fn setting_key(scope: AppScope, field: Field) -> &'static str {
    match (scope, field) {
        (AppScope::Global, Field::ClientId) => "github_app.global.client_id",
        (AppScope::Global, Field::Slug) => "github_app.global.slug",
        (AppScope::Global, Field::PrivateKey) => "github_app.global.private_key",
        (AppScope::Sync, Field::ClientId) => "github_app.sync.client_id",
        (AppScope::Sync, Field::Slug) => "github_app.sync.slug",
        (AppScope::Sync, Field::PrivateKey) => "github_app.sync.private_key",
        (AppScope::Feedback, Field::ClientId) => "github_app.feedback.client_id",
        (AppScope::Feedback, Field::Slug) => "github_app.feedback.slug",
        (AppScope::Feedback, Field::PrivateKey) => "github_app.feedback.private_key",
    }
}

/// Where one field's value came from. Under whole-application resolution the
/// *scope* is the same for all three, but the *source* need not be: an
/// operator may hold a client ID in the environment and a key in the instance
/// store. FR-021 is about being able to see that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSource {
    pub field: Field,
    /// The declaration that supplied it.
    pub key: &'static str,
    pub scope: AppScope,
    /// The environment variable that fixed it, when one did.
    pub fixed_by: Option<&'static str>,
}

/// A resolved application, and the record of how it was assembled.
pub struct ScopedApp {
    pub app: GitHubApp,
    /// The scope whose application answered — `Global` when a subsystem fell
    /// through to it.
    pub scope: AppScope,
    /// The scope that asked.
    pub requested: AppScope,
    pub sources: Vec<FieldSource>,
    /// Non-empty only when the requested scope was **partially** specified and
    /// was therefore stepped over whole (FR-021, US5 scenario 4). Empty when
    /// the subsystem was simply not configured, because "you have not set up a
    /// feedback application" is the ordinary case and not a problem report.
    pub stepped_over: Vec<CredentialProblem>,
}

impl ScopedApp {
    /// True when the answer came from somewhere other than the scope asked.
    pub fn fell_back(&self) -> bool {
        self.scope != self.requested
    }
}

/// Why an application could not be resolved.
///
/// **No variant carries a value, a fragment of one, or its length** (FR-023,
/// SC-007). Every one names declarations an operator can go and set, which is
/// the same contract `settings::registry` keeps and the reason this reports
/// setting keys rather than inventing a second vocabulary of variable names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialProblem {
    Missing {
        field: Field,
        specific: &'static str,
        global: &'static str,
    },
    /// The value was a path, and the path could not be read. The path is named
    /// because an operator chose it and it is not the secret; the file's
    /// contents are never mentioned.
    UnreadableKeyFile {
        key: &'static str,
        path: String,
        detail: String,
    },
    /// `..._PRIVATE_KEY_BASE64` was set to something that is not base64.
    /// Reported against that variable rather than falling through, because the
    /// operator declared an encoding and deserves to be told it was wrong.
    UndecodableBase64 { key: &'static str, detail: String },
    /// Present, and not a key. The case a presence check calls "configured".
    UnreadableKey { key: &'static str, detail: String },
}

impl CredentialProblem {
    /// The setting key an operator should go and look at.
    pub fn key(&self) -> &'static str {
        match self {
            CredentialProblem::Missing { specific, .. } => specific,
            CredentialProblem::UnreadableKeyFile { key, .. }
            | CredentialProblem::UndecodableBase64 { key, .. }
            | CredentialProblem::UnreadableKey { key, .. } => key,
        }
    }

    /// What to do about it, naming declarations and never values.
    pub fn guidance(&self) -> String {
        match self {
            CredentialProblem::Missing {
                field,
                specific,
                global,
            } if specific == global => format!(
                "`{specific}` is not set, so this instance has no {} for that application.",
                field.label()
            ),
            CredentialProblem::Missing {
                field,
                specific,
                global,
            } => format!(
                "Neither `{specific}` nor `{global}` is set, so this instance has no {} \
                 for that application. Set the first to give this subsystem its own \
                 application, or the second to share one.",
                field.label()
            ),
            CredentialProblem::UnreadableKeyFile { key, path, detail } => {
                format!("`{key}` names the file {path}, which could not be read ({detail}).")
            }
            CredentialProblem::UndecodableBase64 { key, detail } => format!(
                "`{key}` was given in its base64 form and is not valid base64 ({detail}). \
                 It must be the PEM file encoded whole — for example `base64 -w0 app-key.pem`."
            ),
            CredentialProblem::UnreadableKey { key, detail } => format!(
                "`{key}` is set but could not be read as an RSA private key ({detail}). \
                 Accepted forms are a PEM, a path to one, or the PEM base64-encoded."
            ),
        }
    }
}

/// Every problem's guidance, as one sentence an operator can act on in a pass.
pub fn join_problems(problems: &[CredentialProblem]) -> String {
    problems
        .iter()
        .map(CredentialProblem::guidance)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The declarations to name when nothing is configured — keys only, never
/// values, which is what makes this safe to render on an operator surface.
pub fn missing_keys(problems: &[CredentialProblem]) -> Vec<&'static str> {
    problems.iter().map(CredentialProblem::key).collect()
}

/// What to tell an operator whose subsystem application was half-written and
/// stepped over whole (FR-021, US5 scenario 4).
///
/// This is a separate sentence from [`CredentialProblem::guidance`] because
/// that one says "neither is set", and here the global one *is* — the operator
/// needs to know their subsystem application was skipped, not that nothing
/// worked.
pub fn stepped_over_guidance(scope: AppScope, problems: &[CredentialProblem]) -> String {
    let names = missing_keys(problems)
        .iter()
        .map(|k| format!("`{k}`"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "The `{}` application is only partly configured — {names} {} not set — so {} is using \
         the global application instead. An application is never completed from another: a \
         client ID from one registration with a private key from another is an authentication \
         failure that reads like a bad key. Set what is missing to give this subsystem its own \
         application, or clear what is set.",
        scope.as_str(),
        if problems.len() == 1 { "is" } else { "are" },
        scope.serves(),
    )
}

/// One scope's application, considered on its own, with nothing borrowed.
struct Candidate {
    /// True when *any* of the three fields is set. The difference between "not
    /// configured" and "half configured", which is the whole of FR-021.
    configured: bool,
    resolved: Result<(GitHubApp, Vec<FieldSource>), Vec<CredentialProblem>>,
}

/// Resolve one scope's application from its own three declarations only.
///
/// Nothing here looks at another scope. That is the property that makes
/// "an application resolves whole" true by construction rather than by
/// discipline.
fn candidate(settings: &Settings, scope: AppScope) -> Candidate {
    let mut problems = Vec::new();
    let mut sources = Vec::new();
    let mut configured = false;

    // Cloned rather than borrowed: the closure already holds `sources` and
    // `configured` mutably, and handing back a reference into `settings` from
    // inside it is a lifetime the inferencer will not commit to.
    let mut take = |field: Field, problems: &mut Vec<CredentialProblem>| -> Option<Resolved> {
        let key = setting_key(scope, field);
        match settings.get(key).filter(|r| r.is_set()) {
            Some(resolved) => {
                configured = true;
                sources.push(FieldSource {
                    field,
                    key,
                    scope,
                    fixed_by: resolved.fixed_by,
                });
                Some(resolved.clone())
            }
            None => {
                problems.push(CredentialProblem::Missing {
                    field,
                    specific: key,
                    global: setting_key(AppScope::Global, field),
                });
                None
            }
        }
    };

    let client_id = take(Field::ClientId, &mut problems).and_then(|r| r.value);
    let slug = take(Field::Slug, &mut problems).and_then(|r| r.value);
    let key_pem = match take(Field::PrivateKey, &mut problems) {
        None => None,
        Some(resolved) => match private_key_bytes(&resolved, setting_key(scope, Field::PrivateKey))
        {
            Ok(bytes) => Some(bytes),
            Err(problem) => {
                problems.push(problem);
                None
            }
        },
    };

    let (Some(client_id), Some(slug), Some(key_pem)) = (client_id, slug, key_pem) else {
        return Candidate {
            configured,
            resolved: Err(problems),
        };
    };

    // Parsed now, not at first use (FR-022).
    let resolved = match GitHubApp::new(client_id, slug, &key_pem) {
        Ok(app) if problems.is_empty() => Ok((app, sources)),
        Ok(_) => Err(problems),
        Err(e) => {
            problems.push(CredentialProblem::UnreadableKey {
                key: setting_key(scope, Field::PrivateKey),
                detail: e.to_string(),
            });
            Err(problems)
        }
    };

    Candidate {
        configured,
        resolved,
    }
}

/// Resolve the application one subsystem should act as.
///
/// Returns **every** problem rather than the first, so an operator fixes their
/// configuration in one pass instead of discovering the next gap after each
/// restart — the posture [`crate::repo_host::registration_from_env`] already
/// takes.
pub fn registration_for(
    scope: AppScope,
    settings: &Settings,
) -> Result<ScopedApp, Vec<CredentialProblem>> {
    let own = candidate(settings, scope);

    let own_problems = match own.resolved {
        Ok((app, sources)) => {
            return Ok(ScopedApp {
                app,
                scope,
                requested: scope,
                sources,
                stepped_over: Vec::new(),
            });
        }
        Err(problems) => problems,
    };

    // The global scope has no fallback of its own — it *is* the fallback.
    if scope == AppScope::Global {
        return Err(own_problems);
    }

    match candidate(settings, AppScope::Global).resolved {
        Ok((app, sources)) => Ok(ScopedApp {
            app,
            scope: AppScope::Global,
            requested: scope,
            sources,
            // Only a *partly* configured subsystem is something to report. A
            // subsystem nobody configured is the ordinary case.
            stepped_over: if own.configured {
                own_problems
            } else {
                Vec::new()
            },
        }),
        // Neither worked. The subsystem's own problems are returned because
        // each `Missing` already names both places a value could go, so the
        // operator is told about the global one without being told about it
        // twice.
        Err(_) => Err(own_problems),
    }
}

/// One scope's own application, with no fallback of any kind.
///
/// The operator surface needs "is *this* application complete" as a separate
/// question from "what will this subsystem use", because the answer to the
/// second can be the global application and rendering that as the subsystem's
/// own is how a screen tells somebody they configured something they did not.
pub fn own_registration(
    scope: AppScope,
    settings: &Settings,
) -> Result<ScopedApp, Vec<CredentialProblem>> {
    let own = candidate(settings, scope);
    match own.resolved {
        Ok((app, sources)) => Ok(ScopedApp {
            app,
            scope,
            requested: scope,
            sources,
            stepped_over: Vec::new(),
        }),
        Err(problems) => Err(problems),
    }
}

/// Whether *any* of a scope's three fields is set. The difference between "not
/// configured" and "half configured", which the surface renders differently.
pub fn is_configured(scope: AppScope, settings: &Settings) -> bool {
    Field::all()
        .iter()
        .any(|field| settings.is_set(setting_key(scope, *field)))
}

/// Which subsystems one scope's application will act for, as things currently
/// resolve (FR-020).
///
/// Computed from [`registration_for`] rather than from a rule written twice:
/// the answer an operator is shown is the answer delivery will get.
pub fn acts_for(scope: AppScope, settings: &Settings) -> Vec<AppScope> {
    AppScope::subsystems()
        .iter()
        .copied()
        .filter(|subsystem| {
            registration_for(*subsystem, settings).is_ok_and(|app| app.scope == scope)
        })
        .collect()
}

/// The PEM bytes behind a resolved private-key setting.
///
/// **The form is decided by the variable that set it, not by the value.** A
/// value that arrived through `..._PRIVATE_KEY_FILE` is a path; one that
/// arrived through `..._PRIVATE_KEY_BASE64` is base64 and a failure to decode
/// is reported against that variable; anything else — including a value stored
/// in the instance's own settings — goes through
/// [`crate::repo_host::normalise_pem`], which accepts a PEM, literal `\n`
/// escapes, or base64 *whose result is a PEM*.
pub fn private_key_bytes(
    resolved: &Resolved,
    key: &'static str,
) -> Result<Vec<u8>, CredentialProblem> {
    let raw = resolved.value.clone().unwrap_or_default();
    let raw = raw.trim();
    match resolved.fixed_by {
        Some(var) if var.ends_with("_PRIVATE_KEY_FILE") => {
            std::fs::read(raw).map_err(|e| CredentialProblem::UnreadableKeyFile {
                key,
                path: raw.to_string(),
                detail: e.to_string(),
            })
        }
        Some(var) if var.ends_with("_PRIVATE_KEY_BASE64") => {
            let compact: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
            general_purpose::STANDARD.decode(&compact).map_err(|e| {
                CredentialProblem::UndecodableBase64 {
                    key,
                    detail: e.to_string(),
                }
            })
        }
        _ => Ok(crate::repo_host::normalise_pem(raw)),
    }
}

/// Parse a private key exactly as resolution will, for the save path (FR-022).
///
/// `setGithubApplication` calls this before storing, so a value that is not a
/// key is refused at the moment it is typed rather than at 3am when a delivery
/// fails. The instance store holds the inline forms only, so this is
/// `normalise_pem` + the parser — the same two steps
/// [`private_key_bytes`] takes for an instance-sourced value.
pub fn parse_private_key_for_storage(raw: &str) -> Result<(), String> {
    let pem = crate::repo_host::normalise_pem(raw.trim());
    // A throwaway identity: `GitHubApp::new` is the only parser, and running
    // the real one is the point — a second, laxer check here would be a value
    // this refuses and resolution accepts, or the reverse.
    GitHubApp::new("validation".to_string(), "validation".to_string(), &pem)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "github_apps_tests.rs"]
mod tests;
