//! The operator's view of this instance's GitHub applications, at both scales.
//!
//! Spec 040 US5, `contracts/github-applications.md`. Resolution itself lives
//! in [`crate::github_apps`]; this file only renders it and writes to it.
//!
//! # The redaction rule, in one sentence
//!
//! **No field, message or log line here carries a private key, a fragment of
//! one, or its length.** `hasPrivateKey` is a boolean; `guidance` names
//! settings; `lastCheckOutcome` carries a status code and never a response
//! body. A length is information about a secret and is excluded deliberately —
//! FR-023 and SC-007 say so, and the test at the bottom of this file asserts
//! it against every string this surface can produce rather than against the
//! ones somebody remembered.
//!
//! # Why `setGithubApplication` exists when `updateInstanceSetting` already
//! writes any declared key
//!
//! It delegates to the same `settings::changes::write_setting` — same
//! validation, same encryption, same audit record — and adds exactly one
//! thing: **the private key is parsed on save, by the parser resolution will
//! use** (FR-022). A credential discovered to be malformed at 3am when a
//! delivery fails is worse than one refused at the moment it is typed. The
//! generic setting mutation cannot do that without the registry knowing what
//! an RSA key is, which is a dependency the registry should not have.
//!
//! # Why the live check is a separate, operator-initiated mutation
//!
//! FR-022 read deliberately (research.md § R10): "validated when configured"
//! is a *parse*, not a network call. A save that requires reachable GitHub
//! cannot be made from a firewalled deployment, and a configuration screen
//! that refuses to store a correct value because a network was down is worse
//! than one that stores it and says it has not been proven yet.

use async_graphql::{Context, Object, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;

use crate::github_apps::{
    AppScope, CredentialProblem, Field, ScopedApp, acts_for, is_configured, missing_keys,
    own_registration, parse_private_key_for_storage, registration_for, setting_key,
    stepped_over_guidance,
};
use crate::graphql::{admin_user, app_state};
use crate::schema::instance_settings;
use crate::settings::changes::{ChangeSource, write_setting};
use crate::settings::graphql::GraphQLSettingSource;
use crate::settings::resolver::{Settings, Source, resolve_all};
use crate::state::AppState;

/// Where the record of the last live check lives.
///
/// Under `settings::registry::RESERVED_PREFIX`, which is the instance's own
/// bookkeeping: such a row does not resolve, is not editable and is not
/// reported as an operator's stale configuration. A declaration would have
/// been wrong — this is not something anybody sets.
fn check_key(scope: AppScope) -> String {
    format!("system.github_app_check.{}", scope.as_str())
}

/// One field of one application, and where its value actually came from.
///
/// FR-021 at the granularity that exists after research.md § R10: an
/// application resolves whole, so the *scope* is the same for all three, but
/// the *source* need not be — an operator may hold a client ID in the
/// environment and a key in the instance store, and a field greyed out with no
/// stated reason is what this feature exists to stop.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "GithubApplicationField")]
pub struct GraphQLApplicationField {
    /// `client_id`, `slug` or `private_key` — the argument `setGithubApplication` takes.
    pub field: String,
    /// The declared setting key, which is what every diagnostic names.
    pub key: String,
    pub set: bool,
    pub source: GraphQLSettingSource,
    /// The environment variable that fixed it, when one did. Never a value.
    pub fixed_by: Option<String>,
    /// False when the environment has fixed it. Do not offer a field (FR-009).
    pub editable: bool,
}

/// One scope's application, as an operator needs to understand it.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "GithubApplication")]
pub struct GraphQLGithubApplication {
    /// `global`, `sync` or `feedback`.
    pub scope: String,
    /// What this application is used for, in a sentence. FR-020: the global
    /// application's scope is an explanation, not a field.
    pub serves: String,
    /// True when at least one of the three fields is set.
    pub configured: bool,
    /// True when all three are set **and the key parses**. A key that is
    /// present and is not a key is the case a presence check calls
    /// "configured", and this is not that check.
    pub complete: bool,
    /// The source of the application as a whole: ENVIRONMENT when any field is
    /// fixed by a variable, otherwise where its values are stored. Null when
    /// nothing is set. Per-field detail is in `fields`.
    pub source: Option<GraphQLSettingSource>,
    /// Not a secret — GitHub publishes it.
    pub client_id: Option<String>,
    pub slug: Option<String>,
    /// **Never the key, never a fragment, never a length.**
    pub has_private_key: bool,
    pub fields: Vec<GraphQLApplicationField>,
    /// Settings this application is missing, by name.
    pub missing: Vec<String>,
    /// Every problem, not the first — the shape `registration_from_env` already
    /// returns, so an operator fixes their configuration in one pass instead of
    /// discovering the next gap after each restart.
    pub guidance: Vec<String>,
    /// Which subsystems this application will act for, as it currently
    /// resolves (FR-020). Computed from the resolution delivery will get, not
    /// from a rule written twice.
    pub acts_for: Vec<String>,
    /// For a subsystem: the scope whose application it will actually use —
    /// its own, or `global`. Null when nothing resolves at all.
    pub resolves_to: Option<String>,
    /// Non-empty when this subsystem is **partly** configured and was
    /// therefore stepped over whole (FR-021). An application is never
    /// completed from another.
    pub stepped_over: Vec<String>,
    /// The same fact as a sentence, for the screen.
    pub stepped_over_guidance: Option<String>,
    pub last_checked_at: Option<String>,
    /// The outcome of the last live check. A status code and a verdict; never
    /// a response body, because a 401 payload is one field away from carrying
    /// a credential.
    pub last_check_outcome: Option<String>,
}

/// Read the record of the last live check for one scope.
async fn last_check(state: &AppState, scope: AppScope) -> Option<(String, String)> {
    let key = check_key(scope);
    let mut conn = state.db_pool.get().ok()?;
    let raw = tokio::task::spawn_blocking(move || {
        instance_settings::table
            .filter(instance_settings::key.eq(key))
            .select(instance_settings::value)
            .first::<String>(&mut conn)
            .optional()
            .ok()
            .flatten()
    })
    .await
    .ok()
    .flatten()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    Some((
        parsed.get("at")?.as_str()?.to_string(),
        parsed.get("outcome")?.as_str()?.to_string(),
    ))
}

async fn record_check(state: &AppState, scope: AppScope, outcome: &str) -> String {
    let at = chrono::Utc::now();
    let json = serde_json::json!({ "at": at.to_rfc3339(), "outcome": outcome }).to_string();
    let key = check_key(scope);
    if let Ok(mut conn) = state.db_pool.get() {
        let now = at.naive_utc();
        let _ = tokio::task::spawn_blocking(move || {
            diesel::insert_into(instance_settings::table)
                .values((
                    instance_settings::key.eq(&key),
                    instance_settings::value.eq(&json),
                    instance_settings::updated_at.eq(now),
                    instance_settings::created_at.eq(now),
                ))
                .on_conflict(instance_settings::key)
                .do_update()
                .set((
                    instance_settings::value.eq(&json),
                    instance_settings::updated_at.eq(now),
                ))
                .execute(&mut conn)
        })
        .await;
    }
    at.to_rfc3339()
}

/// Render one scope. The whole of this surface's redaction rule is here: the
/// private key is read only to answer `has_private_key`, and its value never
/// leaves this function.
fn describe(
    scope: AppScope,
    settings: &Settings,
    check: Option<(String, String)>,
) -> GraphQLGithubApplication {
    let fields: Vec<GraphQLApplicationField> = Field::all()
        .iter()
        .map(|field| {
            let key = setting_key(scope, *field);
            let resolved = settings.get(key);
            GraphQLApplicationField {
                field: field.as_str().to_string(),
                key: key.to_string(),
                set: resolved.is_some_and(|r| r.is_set()),
                source: resolved.map(|r| r.source).unwrap_or(Source::Default).into(),
                fixed_by: resolved.and_then(|r| r.fixed_by).map(str::to_string),
                editable: resolved.is_none_or(|r| r.editable()),
            }
        })
        .collect();

    let own = own_registration(scope, settings);
    let problems: Vec<CredentialProblem> = own.as_ref().err().cloned().unwrap_or_default();
    let resolution = registration_for(scope, settings).ok();

    let any_environment = fields
        .iter()
        .any(|f| f.set && f.source == GraphQLSettingSource::Environment);
    let any_set = fields.iter().any(|f| f.set);

    GraphQLGithubApplication {
        scope: scope.as_str().to_string(),
        serves: scope.serves().to_string(),
        configured: is_configured(scope, settings),
        complete: own.is_ok(),
        source: match (any_set, any_environment) {
            (false, _) => None,
            (true, true) => Some(GraphQLSettingSource::Environment),
            (true, false) => Some(GraphQLSettingSource::Instance),
        },
        client_id: settings
            .value(setting_key(scope, Field::ClientId))
            .map(str::to_string),
        slug: settings
            .value(setting_key(scope, Field::Slug))
            .map(str::to_string),
        // A boolean, and only ever a boolean.
        has_private_key: settings.is_set(setting_key(scope, Field::PrivateKey)),
        fields,
        // This scope's own unset settings. Factual whatever else is true — a
        // subsystem using the global application still has three of its own
        // that are unset, and the screen renders that as an offer rather than
        // as a demand.
        missing: missing_keys(&problems)
            .into_iter()
            .map(str::to_string)
            .collect(),
        // **Guidance is about the resolution, not about the scope's own
        // fields**, and getting that wrong was a real bug here: an
        // unconfigured `feedback` scope beside a working global application
        // rendered "Neither `github_app.feedback.client_id` nor
        // `github_app.global.client_id` is set" — with the second one plainly
        // set on the same screen. `Missing::guidance` names both places a
        // value could go, which is right when nothing resolves and wrong the
        // moment something does.
        //
        // So: a failed resolution says what is missing; a resolution that fell
        // through from a *half-written* subsystem says that instead; and one
        // that simply works says nothing, because an instance with one
        // application configured has no gap to report.
        guidance: match (&resolution, problems.is_empty()) {
            (None, _) => problems.iter().map(CredentialProblem::guidance).collect(),
            (Some(_), false) if !stepped_over_problems(&resolution).is_empty() => {
                vec![stepped_over_guidance(
                    scope,
                    stepped_over_problems(&resolution),
                )]
            }
            _ => Vec::new(),
        },
        acts_for: acts_for(scope, settings)
            .into_iter()
            .map(|s| s.as_str().to_string())
            .collect(),
        resolves_to: resolution
            .as_ref()
            .map(|app: &ScopedApp| app.scope.as_str().to_string()),
        stepped_over: resolution
            .as_ref()
            .map(|app| {
                missing_keys(&app.stepped_over)
                    .into_iter()
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        stepped_over_guidance: resolution.as_ref().and_then(|app| {
            (!app.stepped_over.is_empty()).then(|| stepped_over_guidance(scope, &app.stepped_over))
        }),
        last_checked_at: check.as_ref().map(|(at, _)| at.clone()),
        last_check_outcome: check.map(|(_, outcome)| outcome),
    }
}

/// The problems a fallback stepped over, or nothing.
fn stepped_over_problems(resolution: &Option<ScopedApp>) -> &[CredentialProblem] {
    resolution
        .as_ref()
        .map(|app| app.stepped_over.as_slice())
        .unwrap_or(&[])
}

fn scope_arg(scope: &str) -> GraphQLResult<AppScope> {
    AppScope::parse(scope).ok_or_else(|| {
        async_graphql::Error::new(format!(
            "`{scope}` is not a scope this instance has. The scopes are: global, sync, feedback."
        ))
    })
}

#[derive(Default)]
pub struct GithubAppQuery;

#[Object]
impl GithubAppQuery {
    /// Every scope, plus how each subsystem currently resolves. Administrators
    /// only — a client ID is not a secret, but which applications an instance
    /// holds is not a stranger's business either.
    async fn github_applications(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<Vec<GraphQLGithubApplication>> {
        admin_user(ctx)?;
        let state = app_state(ctx)?;
        let settings = resolve_all(state)
            .await
            .map_err(async_graphql::Error::new)?;

        let mut out = Vec::new();
        for scope in AppScope::all() {
            let check = last_check(state, *scope).await;
            out.push(describe(*scope, &settings, check));
        }
        Ok(out)
    }
}

#[derive(Default)]
pub struct GithubAppMutation;

#[Object]
impl GithubAppMutation {
    /// Set or clear one field of one scope's application.
    ///
    /// The private key is parsed on save, by the parser resolution uses
    /// (FR-022). A value that is not a key is refused now, naming the field,
    /// and is **never stored** — an application is not "configured" because a
    /// presence check found a string.
    ///
    /// The `_FILE` and `_BASE64` forms are environment forms and stay that
    /// way; see the note in `.env.example`. A key pasted here in its base64
    /// form is still accepted, because `normalise_pem` accepts base64 *whose
    /// result is a PEM* and an operator who has that string in their clipboard
    /// should not have to decode it by hand.
    async fn set_github_application(
        &self,
        ctx: &Context<'_>,
        scope: String,
        field: String,
        value: Option<String>,
    ) -> GraphQLResult<GraphQLGithubApplication> {
        let user = admin_user(ctx)?;
        let state = app_state(ctx)?;
        let scope = scope_arg(&scope)?;
        let field = Field::parse(&field).ok_or_else(|| {
            async_graphql::Error::new(format!(
                "`{field}` is not a field of a GitHub application. The fields are: \
                 client_id, slug, private_key."
            ))
        })?;
        let key = setting_key(scope, field);

        // FR-022, at the moment the value is typed. Before the write, so a
        // refusal leaves the previous value in place rather than replacing a
        // working credential with a broken one.
        if field == Field::PrivateKey
            && let Some(raw) = value.as_deref().filter(|v| !v.trim().is_empty())
            && let Err(detail) = parse_private_key_for_storage(raw)
        {
            // Names the setting and the reason, and quotes nothing back.
            return Err(async_graphql::Error::new(format!(
                "`{key}` was not stored: the value could not be read as an RSA private key \
                 ({detail}). Accepted forms are the PEM itself, a PEM with literal \\n \
                 escapes, or the PEM base64-encoded. A mounted secret file is set with the \
                 matching `..._PRIVATE_KEY_FILE` environment variable instead, so the file \
                 stays the only place the key lives."
            )));
        }

        write_setting(
            state,
            key,
            value.as_deref(),
            Some(user.user_id),
            ChangeSource::Admin,
        )
        .await
        .map_err(async_graphql::Error::new)?;

        let settings = resolve_all(state)
            .await
            .map_err(async_graphql::Error::new)?;
        let check = last_check(state, scope).await;
        Ok(describe(scope, &settings, check))
    }

    /// Ask the host whether the application this scope resolves to actually
    /// works, and record the answer.
    ///
    /// One app-authenticated call — the assertion this application would sign
    /// anyway — against the installations endpoint. Nothing is written
    /// anywhere and no installation is touched; a 200 means the registration
    /// and its key are genuinely accepted, which is the one thing a parse
    /// cannot tell an operator.
    async fn check_github_application(
        &self,
        ctx: &Context<'_>,
        scope: String,
    ) -> GraphQLResult<GraphQLGithubApplication> {
        admin_user(ctx)?;
        let state = app_state(ctx)?;
        let scope = scope_arg(&scope)?;
        let settings = resolve_all(state)
            .await
            .map_err(async_graphql::Error::new)?;

        let outcome = match registration_for(scope, &settings) {
            Err(_) => "not configured — nothing to check".to_string(),
            Ok(app) => live_check(&app).await,
        };
        let at = record_check(state, scope, &outcome).await;
        Ok(describe(scope, &settings, Some((at, outcome))))
    }
}

/// One app-authenticated request, reduced to a sentence with a status code in
/// it and nothing else.
///
/// **No response body ever reaches the outcome string.** `repo_host::scoped`
/// takes the same posture for delivery failures and gives the reason: a
/// free-text field carrying a host body is one 401 payload away from
/// disclosing a credential, invisibly, on a day nobody is looking.
async fn live_check(app: &ScopedApp) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let Ok(assertion) = app.app.app_assertion(now) else {
        return "the private key could not sign an assertion".to_string();
    };
    let Ok(client) = reqwest::Client::builder()
        .user_agent("ThunderForgeVTT")
        .build()
    else {
        return "no HTTP client could be built".to_string();
    };

    match client
        .get(app.app.installations_url())
        .bearer_auth(&assertion)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
    {
        Err(_) => "the host could not be reached".to_string(),
        Ok(response) => {
            let status = response.status().as_u16();
            match status {
                200..=299 => format!("accepted by the host ({status})"),
                401 => "rejected by the host (401) — the client ID and the private key do not \
                        belong to the same application, or the key has been revoked"
                    .to_string(),
                404 => "not found by the host (404) — check the client ID; it is not the slug"
                    .to_string(),
                other => format!("refused by the host ({other})"),
            }
        }
    }
}

#[cfg(test)]
#[path = "mutations_github_apps_tests.rs"]
mod tests;
