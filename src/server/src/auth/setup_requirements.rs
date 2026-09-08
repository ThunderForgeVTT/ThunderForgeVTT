//! What setup still needs, and the one predicate that decides whether it may
//! finish (spec 040 US1, FR-002, FR-002a, FR-003; `contracts/setup.md` rule 4).
//!
//! # Why this is not a list of steps
//!
//! The wizard used to be a hard-coded sequence of screens. It is now driven by
//! `settings::registry`: **every declaration marked `RequiredAtSetup` is a
//! thing setup asks for, and nothing else is.** Adding a declaration adds a
//! field to setup with no change here and no change in `SetupPage.tsx`. That
//! is FR-012 reaching the interface, and it is why nothing in this file names
//! `operator.name` — a predicate that spelled the keys out would be a second
//! declaration list that could disagree with the first.
//!
//! # Why it lives in its own file
//!
//! `admin_setup.rs` holds the handlers. The predicate is the part with the
//! interesting rules and the part worth testing without an HTTP request in
//! sight, and separating it also keeps `admin_setup.rs` under the 1000-line
//! gate `scripts/check-file-length.sh` enforces.
//!
//! # The second factor (FR-002a)
//!
//! Setup does not complete while the first administrator's
//! `two_factor_confirmed_at` is null. **Only the predicate is 040's**; the
//! enrolment flow is spec 041's and is already built (`/settings/security`,
//! and the login-challenge entrance). research.md § D1 is the reasoning for
//! landing the gate before the pleasant way to satisfy it.

use diesel::prelude::*;
use serde::Serialize;

use crate::schema::users;
use crate::settings::declarations;
use crate::settings::registry::{Capability, Kind, Requirement, SettingDeclaration};
use crate::settings::resolver::{Settings, Source, resolve_all};
use crate::state::AppState;

/// One setting the wizard asks about, resolved.
///
/// # Two different questions
///
/// **What setup asks about** and **what blocks completion** are not the same
/// list, and FR-003 exists to keep them apart: "setup MUST distinguish what it
/// requires from what it merely offers, and MUST complete without the optional
/// parts."
///
/// - Asked about: every `RequiredAtSetup` declaration, plus the
///   `RequiredFor(capability)` declarations for the three capabilities FR-002
///   names in its one pass — the copyright-notice contact, mail delivery, and
///   the operator's jurisdiction. Getting this wrong once already produced a
///   wizard with no notices step and no mail step, so FR-002's own list was
///   never collected.
/// - Blocks completion: `RequiredAtSetup` alone. An instance may finish setup
///   with no mail server; it simply cannot send mail, and the review step's
///   readiness report says so (FR-003, FR-005 scenario 5).
///
/// `requirement` is the discriminator the wizard reads to tell a step it may
/// skip from one it may not.
///
/// # What is deliberately not asked about
///
/// `SyncLore` and the GitHub applications. They belong to US5, and FR-002 does
/// not name them in the one pass — a first run that demanded a GitHub App
/// registration before it would finish would be a worse first run.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct RequiredSetting {
    pub(crate) key: &'static str,
    /// `TEXT`, `EMAIL`, `URL`, `PORT`, `BOOL`, `ENUM`, `PROSE`.
    pub(crate) kind: &'static str,
    /// `REQUIRED_AT_SETUP` — setup does not finish without it.
    /// `REQUIRED_FOR` — offered, skippable, and a readiness gap while unset.
    /// `OPTIONAL` — offered and nothing depends on it.
    pub(crate) requirement: &'static str,
    /// The capability this contributes to, for the review step's readiness
    /// report to key against. `None` for a setting that enables nothing in
    /// particular.
    pub(crate) capability: Option<&'static str>,
    pub(crate) satisfied: bool,
    /// `ENVIRONMENT` / `INSTANCE` / `DEFAULT`, or null while unset.
    ///
    /// Upper case because `contracts/setup.md` writes it that way and the
    /// wizard is built against that document. The resolver's own vocabulary is
    /// the same three words in lower case (`Source::as_str`); this is a
    /// rendering of it, not a second one.
    pub(crate) source: Option<&'static str>,
    pub(crate) fixed_by: Option<&'static str>,
    /// The value already stored, so a resumed pass shows what was entered
    /// rather than a blank box (FR-006).
    ///
    /// **Only for a value the instance itself holds, and never for a secret.**
    /// An environment-fixed value is reported as fixed and not echoed — the
    /// wizard does not offer a field for it (FR-009) so it has nothing to
    /// pre-fill — and a declaration marked secret has exactly two renderings
    /// anywhere in this product, `set` and `not set`. This endpoint answers
    /// without a session, which makes it the last place to make an exception.
    pub(crate) value: Option<String>,
    pub(crate) what_to_set: &'static str,
    pub(crate) what_is_limited: &'static str,
    pub(crate) group: &'static str,
}

/// The capabilities FR-002's one pass collects for, beyond the declarations
/// that are `RequiredAtSetup` outright.
///
/// Named here rather than inferred, because "which capabilities does first run
/// ask about" is a product decision from the requirement text and not a
/// property of the registry. Adding a capability to the product does not
/// silently add a step to the wizard.
const CAPABILITIES_SETUP_ASKS_ABOUT: [Capability; 3] = [
    // "the contact for copyright notices"
    Capability::PublishBeyondWorld,
    // "mail delivery settings"
    Capability::SendMail,
    // part of "the operator identity" — the jurisdiction the terms are read
    // under.
    Capability::PublishTerms,
];

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Text => "TEXT",
        Kind::Email => "EMAIL",
        Kind::Url => "URL",
        Kind::Port => "PORT",
        Kind::Bool => "BOOL",
        Kind::Enum(_) => "ENUM",
        Kind::Prose => "PROSE",
    }
}

fn source_name(source: Source) -> &'static str {
    match source {
        Source::Environment => "ENVIRONMENT",
        Source::Instance => "INSTANCE",
        Source::Default => "DEFAULT",
    }
}

fn requirement_name(requirement: Requirement) -> &'static str {
    match requirement {
        Requirement::Optional => "OPTIONAL",
        Requirement::RequiredAtSetup => "REQUIRED_AT_SETUP",
        Requirement::RequiredFor(_) => "REQUIRED_FOR",
    }
}

/// Whether the wizard asks about this declaration at all.
fn setup_asks_about(d: &SettingDeclaration) -> bool {
    if d.is_required_at_setup() {
        return true;
    }
    // An optional declaration is offered when it belongs to a capability the
    // pass covers — an SMTP username is not required, but somebody configuring
    // a mail server needs to be able to enter one in the same breath.
    d.capability
        .is_some_and(|c| CAPABILITIES_SETUP_ASKS_ABOUT.contains(&c))
}

/// Every setting the wizard asks about, resolved, in declaration order.
///
/// Declaration order is the order an operator reads them in — `registry.rs`
/// says so — so the wizard's field order and its step order are the registry's
/// and not a second opinion about it.
pub(crate) fn required_settings(settings: &Settings) -> Vec<RequiredSetting> {
    declarations()
        .iter()
        .filter(|d| setup_asks_about(d))
        .map(|d| {
            let resolved = settings.get(d.key);
            let satisfied = resolved.is_some_and(|r| r.is_set());
            let source = resolved.filter(|_| satisfied).map(|r| r.source);
            RequiredSetting {
                key: d.key,
                kind: kind_name(d.kind),
                requirement: requirement_name(d.requirement),
                capability: d.capability.map(|c| c.key()),
                satisfied,
                // A source is only meaningful for a value that exists. An
                // unset setting resolves with `Source::Default` and reporting
                // that would tell the wizard "this came from the default" for
                // something that has no default at all.
                source: source.map(source_name),
                fixed_by: resolved.and_then(|r| r.fixed_by),
                value: match (source, d.secret) {
                    (Some(Source::Instance), false) => resolved.and_then(|r| r.value.clone()),
                    _ => None,
                },
                what_to_set: d.what_to_set,
                what_is_limited: d.what_is_limited,
                group: d.group,
            }
        })
        .collect()
}

/// The `RequiredAtSetup` keys still unset — **the completion predicate's half**,
/// deliberately narrower than what the wizard asks about.
///
/// Derived from the registry rather than from `required_settings` above, so
/// widening what setup offers can never widen what it refuses to finish
/// without. That coupling is exactly what went wrong the first time these two
/// were one list.
pub(crate) fn missing_required_settings(settings: &Settings) -> Vec<&'static str> {
    declarations()
        .iter()
        .filter(|d| d.is_required_at_setup())
        .filter(|d| !settings.is_set(d.key))
        .map(|d| d.key)
        .collect()
}

/// The completion predicate: everything `/complete` has to be true.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetupCompletion {
    /// `RequiredAtSetup` declarations that do not resolve (FR-002, FR-003).
    pub(crate) missing: Vec<&'static str>,
    /// FR-002a. False when there is no administrator yet — an instance with
    /// nobody in it has not enrolled anybody's second factor either, and
    /// answering "true" for the empty case would let `/complete` run before
    /// the account step.
    pub(crate) second_factor_confirmed: bool,
}

/// Why setup may not finish, or nothing.
///
/// The order is deliberate: unset values are reported before the second
/// factor, because an operator who is missing both should be sent back to the
/// fields they can fill in from where they are standing rather than to an
/// authenticator app and then back again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SetupRefusal {
    /// `409 { code: "incomplete", missing: [...] }`
    Incomplete(Vec<&'static str>),
    /// `409 { code: "second_factor_required" }`
    SecondFactorRequired,
}

impl SetupCompletion {
    pub(crate) fn refusal(&self) -> Option<SetupRefusal> {
        if !self.missing.is_empty() {
            return Some(SetupRefusal::Incomplete(self.missing.clone()));
        }
        if !self.second_factor_confirmed {
            return Some(SetupRefusal::SecondFactorRequired);
        }
        None
    }
}

/// Whether the **first** administrator has a confirmed second factor.
///
/// "First" is `created_at` then `id`, not "any" and not "the caller". An
/// instance's first administrator is the account setup created; asking whether
/// *some* administrator is enrolled would let a second administrator added
/// later satisfy a gate the first one never passed.
///
/// Takes a connection rather than an `AppState` so it can be tested inside a
/// `test_transaction` — see this module's tests for why that matters on a
/// shared development database.
pub(crate) fn first_administrator_second_factor_confirmed(
    conn: &mut PgConnection,
) -> Result<bool, diesel::result::Error> {
    let confirmed = users::table
        .filter(users::is_admin.eq(true))
        .order((users::created_at.asc(), users::id.asc()))
        .select(users::two_factor_confirmed_at)
        .first::<Option<chrono::NaiveDateTime>>(conn)
        .optional()?;

    // `None` is "there is no administrator"; `Some(None)` is "there is one and
    // they have not confirmed". Both are false, and they are different facts,
    // which is why the flatten is written out rather than defaulted.
    Ok(matches!(confirmed, Some(Some(_))))
}

/// The whole predicate, for one request.
pub(crate) async fn evaluate(state: &AppState) -> Result<SetupCompletion, String> {
    let settings = resolve_all(state).await?;
    let missing = missing_required_settings(&settings);

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    let second_factor_confirmed =
        tokio::task::spawn_blocking(move || first_administrator_second_factor_confirmed(&mut conn))
            .await
            .map_err(|_| "Failed to spawn blocking task".to_string())?
            .map_err(|_| "Failed to read the first administrator's second factor".to_string())?;

    Ok(SetupCompletion {
        missing,
        second_factor_confirmed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::test_env::temp_env;
    use crate::test_support::test_app_state;

    /// The three environment variables behind the `RequiredAtSetup`
    /// declarations, derived from the registry rather than typed out — a test
    /// that hard-coded them would pass while the predicate it is guarding
    /// silently stopped covering a fourth declaration.
    fn required_env_vars() -> Vec<&'static str> {
        declarations()
            .iter()
            .filter(|d| d.is_required_at_setup())
            .filter_map(|d| d.env_var)
            .collect()
    }

    /// T053 / FR-002, FR-003. The refusal names the keys, and the keys come
    /// from the registry.
    #[test]
    fn completion_is_refused_while_a_required_declaration_is_unset() {
        let state = test_app_state();

        // Cleared rather than assumed: a developer with
        // THUNDERFORGE_OPERATOR_NAME exported would otherwise see this test
        // pass for the wrong reason.
        let cleared: Vec<(&str, Option<&str>)> =
            required_env_vars().into_iter().map(|v| (v, None)).collect();

        let settings = temp_env_async(&cleared, || resolve_all(&state));

        let missing = missing_required_settings(&settings);
        let expected: Vec<&'static str> = declarations()
            .iter()
            .filter(|d| d.is_required_at_setup())
            .filter(|d| !settings.is_set(d.key))
            .map(|d| d.key)
            .collect();

        assert_eq!(
            missing, expected,
            "the missing list is not the registry's own answer"
        );
        assert!(
            !missing.is_empty(),
            "no RequiredAtSetup declaration is unset on a bare instance, so this \
             test cannot observe the refusal it exists for"
        );

        let completion = SetupCompletion {
            missing: missing.clone(),
            // True, so the refusal under test cannot be the second factor's.
            second_factor_confirmed: true,
        };
        assert_eq!(
            completion.refusal(),
            Some(SetupRefusal::Incomplete(missing)),
            "setup would have completed with a required declaration unset"
        );
    }

    /// FR-009 reaching first run: a value the environment fixes is satisfied,
    /// says so, and names the variable — so the wizard does not ask for it.
    #[test]
    fn an_environment_fixed_requirement_is_satisfied_and_names_its_variable() {
        let state = test_app_state();

        let Some(var) = required_env_vars().first().copied() else {
            panic!("no RequiredAtSetup declaration has an environment form");
        };
        let key = declarations()
            .iter()
            .find(|d| d.is_required_at_setup() && d.env_var == Some(var))
            .expect("the declaration the variable came from")
            .key;

        let settings = temp_env_async(&[(var, Some("A Real Operator"))], || resolve_all(&state));

        let reported = required_settings(&settings)
            .into_iter()
            .find(|r| r.key == key)
            .expect("the requirement is reported");

        assert!(reported.satisfied, "an environment-fixed value is not set");
        assert_eq!(reported.source, Some("ENVIRONMENT"));
        assert_eq!(reported.fixed_by, Some(var));
        assert!(
            !missing_required_settings(&settings).contains(&key),
            "setup would still ask for a value the environment has fixed"
        );
    }

    /// T054 / FR-002a. Nothing missing, no confirmed second factor: still
    /// refused, and refused with the code the contract names.
    #[test]
    fn completion_is_refused_while_the_second_factor_is_unconfirmed() {
        let completion = SetupCompletion {
            missing: Vec::new(),
            second_factor_confirmed: false,
        };
        assert_eq!(
            completion.refusal(),
            Some(SetupRefusal::SecondFactorRequired),
            "setup would have completed without a confirmed second factor"
        );

        let enrolled = SetupCompletion {
            missing: Vec::new(),
            second_factor_confirmed: true,
        };
        assert_eq!(enrolled.refusal(), None);
    }

    /// T054, at the database. The subtle half of FR-002a is *which*
    /// administrator: a later administrator who happens to be enrolled must
    /// not satisfy a gate the first one never passed.
    ///
    /// Runs inside `test_transaction`, which always rolls back. The
    /// development database this suite runs against has three hundred
    /// administrators in it, so "there is no administrator yet" cannot be
    /// arranged by inserting — only by clearing the flag, which is exactly
    /// what must never be committed. Attempted first with a plain pooled
    /// connection and a cleanup step; it left the flag cleared whenever an
    /// assertion failed, which is worse than the bug being tested for.
    #[test]
    fn only_the_first_administrator_satisfies_the_second_factor_gate() {
        let _env = crate::settings::test_env::lock();
        let _rows = super::super::admin_setup::setup_state_lock();
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("a connection");

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            diesel::update(users::table.filter(users::is_admin.eq(true)))
                .set(users::is_admin.eq(false))
                .execute(conn)?;

            assert!(
                !first_administrator_second_factor_confirmed(conn)?,
                "an instance with no administrator reported an enrolled one"
            );

            let earlier = insert_admin(conn, 1, None);
            let later = insert_admin(conn, 2, Some(chrono::Utc::now().naive_utc()));

            assert!(
                !first_administrator_second_factor_confirmed(conn)?,
                "a later administrator's second factor satisfied the gate for the first"
            );

            diesel::update(users::table.filter(users::id.eq(earlier)))
                .set(users::two_factor_confirmed_at.eq(Some(chrono::Utc::now().naive_utc())))
                .execute(conn)?;

            assert!(
                first_administrator_second_factor_confirmed(conn)?,
                "the first administrator's confirmed second factor did not satisfy the gate"
            );

            let _ = later;
            Ok(())
        });
    }

    /// An administrator with a deterministic creation order, so "first" is a
    /// fact and not a race between two `now_v7`s in the same millisecond.
    fn insert_admin(
        conn: &mut PgConnection,
        minutes: i64,
        confirmed_at: Option<chrono::NaiveDateTime>,
    ) -> uuid::Uuid {
        let id = uuid::Uuid::now_v7();
        let created_at =
            chrono::DateTime::UNIX_EPOCH.naive_utc() + chrono::Duration::minutes(minutes);
        diesel::insert_into(users::table)
            .values((
                users::id.eq(id),
                users::username.eq(format!("setup_admin_{}", id.simple())),
                users::email.eq(format!("setup_admin_{}@example.invalid", id.simple())),
                users::password_hash.eq("not-a-real-hash"),
                users::is_admin.eq(true),
                users::created_at.eq(created_at),
                users::updated_at.eq(created_at),
                users::two_factor_confirmed_at.eq(confirmed_at),
            ))
            .execute(conn)
            .expect("failed to insert a test administrator");
        id
    }

    /// The distinction FR-003 exists for, and the one this module got wrong
    /// first time round: **what setup asks about is wider than what blocks
    /// completion.**
    ///
    /// A `RequiredFor` declaration that is unset — a mail host, a notice
    /// contact — is offered by the wizard and must **not** appear in the
    /// missing list. An instance may finish setup with no mail server; it
    /// simply cannot send mail, and readiness says so.
    #[test]
    fn what_setup_offers_is_wider_than_what_blocks_completion() {
        let state = test_app_state();
        let cleared: Vec<(&str, Option<&str>)> = declarations()
            .iter()
            .filter_map(|d| d.env_var)
            .map(|v| (v, None))
            .collect();
        let settings = temp_env_async(&cleared, || resolve_all(&state));

        let offered: Vec<&'static str> = required_settings(&settings)
            .into_iter()
            .map(|r| r.key)
            .collect();
        let blocking = missing_required_settings(&settings);

        for key in &blocking {
            assert!(
                offered.contains(key),
                "`{key}` blocks completion but setup never asks for it"
            );
        }

        for entry in required_settings(&settings) {
            if entry.requirement != "REQUIRED_AT_SETUP" {
                assert!(
                    !blocking.contains(&entry.key),
                    "`{}` is skippable but was treated as blocking completion",
                    entry.key
                );
            }
        }

        // FR-002's own list, which the wizard renders as steps: the copyright
        // notice contact, mail delivery, and the operator's jurisdiction.
        // Named by capability rather than by key, so adding a declaration to
        // one of them does not need this test edited.
        for capability in ["publish_beyond_world", "send_mail", "publish_terms"] {
            assert!(
                required_settings(&settings)
                    .iter()
                    .any(|r| r.capability == Some(capability)),
                "setup asks nothing about `{capability}`, so FR-002 never collects it"
            );
        }
    }

    /// A secret is never echoed back, and neither is an environment-fixed
    /// value. This endpoint answers without a session.
    #[test]
    fn no_prefilled_value_is_a_secret_or_an_environment_value() {
        let state = test_app_state();
        let settings = temp_env_async(&[], || resolve_all(&state));

        for entry in required_settings(&settings) {
            let d = crate::settings::registry::declaration(entry.key).expect("declared");
            if entry.value.is_some() {
                assert!(!d.secret, "`{}` echoed a secret back to setup", entry.key);
                assert_eq!(
                    entry.source,
                    Some("INSTANCE"),
                    "`{}` pre-filled a value the instance does not itself hold",
                    entry.key
                );
            }
        }
    }

    /// `settings::test_env::temp_env` takes a synchronous body, and
    /// `resolve_all` is async. Blocking on the future inside the body keeps
    /// the environment lock held for exactly as long as the resolution reads
    /// the environment, which is the property the lock exists for — spawning
    /// the resolution and awaiting it outside would restore the variables
    /// while it was still running.
    fn temp_env_async<F, Fut>(vars: &[(&str, Option<&str>)], body: F) -> Settings
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Settings, String>>,
    {
        let mut out = None;
        temp_env(vars, || {
            out = Some(
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("a runtime")
                    .block_on(body())
                    .expect("settings resolve"),
            );
        });
        out.expect("the body ran")
    }
}
