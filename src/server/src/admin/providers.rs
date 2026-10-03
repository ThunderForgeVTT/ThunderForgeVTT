//! Reading and writing the sign-in providers an operator may configure.
//!
//! Split out of `admin.rs` when that file crossed the length the repo
//! enforces. The seam is a real one: everything here is about one table,
//! `oauth_providers`, and about the two rules that govern it — ADR-041, which
//! says a row the environment configures accepts nothing but `enabled`, and
//! spec 064 FR-032/FR-033, which say a provider whose endpoints come from an
//! issuer is configurable from that issuer alone and that the issuer passes
//! the same outbound-URL guard as any other operator-supplied URL.

use crate::config::oauth_env::{parse_oauth_env_vars, resolve};
use crate::models::{NewOAuthProvider, OAuthProvider};
use crate::schema::oauth_providers;
use crate::state::{AppState, DbPool};
use chrono::Utc;
use diesel::prelude::*;
use std::collections::HashSet;
use thunderforge_axum_oauth::provider_kind::ProviderKind;

pub async fn load_oauth_providers(state: &AppState) -> Result<Vec<OAuthProvider>, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        oauth_providers::table
            .order((
                oauth_providers::display_name.asc(),
                oauth_providers::provider_key.asc(),
            ))
            .select(OAuthProvider::as_select())
            .load::<OAuthProvider>(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to query OAuth providers".to_string())
}

#[derive(Debug, Clone, Default)]
pub struct OAuthProviderUpdate {
    pub display_name: Option<String>,
    pub oauth_client_id: Option<String>,
    pub oauth_client_secret: Option<String>,
    pub enabled: Option<bool>,
    pub userinfo_url: Option<String>,
    pub scopes: Option<Vec<String>>,
    /// The base URL of a self-hosted identity provider, for a kind whose
    /// endpoints are derived from one. Supplying it for a fixed-endpoint kind
    /// is refused rather than ignored — an operator who typed a URL into a
    /// field deserves to be told it does nothing.
    pub issuer_url: Option<String>,
}

pub async fn update_oauth_provider(
    state: &AppState,
    provider_id: uuid::Uuid,
    update: OAuthProviderUpdate,
) -> Result<OAuthProvider, String> {
    // SSRF guard. `userinfo_url` is the one URL on this row an administrator
    // can set through the API, and the server **fetches it**, with the
    // provider access token attached (`fetch_userinfo`). Unchecked, that is a
    // request the instance makes to any address an admin names — and the
    // addresses worth naming are all internal: `169.254.169.254` hands back
    // cloud instance credentials, loopback reaches admin interfaces bound
    // there deliberately.
    //
    // "Only an admin can set it" is weaker than it sounds. An administrator of
    // a ThunderForge instance is not necessarily trusted with the machine it
    // runs on — on anything hosted they are usually different people — and an
    // admin account is a thing that gets taken over. This turns that from a
    // ThunderForge problem into an infrastructure one.
    //
    // Loopback is permitted only in a debug build, for a developer running a
    // provider locally. A release binary does not contain the allowance at
    // all, so no environment variable can switch it on — the same two-locks
    // reasoning `rate_limit_disabled` uses, and for a comparable reason.
    if let Some(url) = update.userinfo_url.as_deref().map(str::trim)
        && !url.is_empty()
    {
        thunderforge_axum_oidc::url_guard::check_outbound_url(url, cfg!(debug_assertions))
            .map_err(|refusal| refusal.message().to_string())?;
    }

    // An issuer goes through the same guard, and has more riding on it: the
    // three endpoints are *derived* from it, so one unchecked base URL buys
    // an attacker three addresses this server will talk to, one of them with
    // an access token attached.
    if let Some(url) = update.issuer_url.as_deref().map(str::trim)
        && !url.is_empty()
    {
        thunderforge_axum_oidc::url_guard::check_outbound_url(url, cfg!(debug_assertions))
            .map_err(|refusal| refusal.message().to_string())?;
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    let now = Utc::now().naive_utc();

    tokio::task::spawn_blocking(move || {
        let existing = oauth_providers::table
            .filter(oauth_providers::id.eq(provider_id))
            .select(OAuthProvider::as_select())
            .first::<OAuthProvider>(&mut conn)
            .optional()?;

        let Some(existing) = existing else {
            return Ok::<Result<Option<OAuthProvider>, String>, diesel::result::Error>(Ok(None));
        };

        // ADR-041 write guard: env-sourced rows are re-asserted by the
        // startup materialization scan on every restart, so admin edits to
        // their credential/URL/label fields would silently have no lasting
        // effect. Only `enabled` (FR-006) is ever writable on such a row
        // through this mutation — every other field in `update` is ignored,
        // not erased, so the response still reflects the row's real,
        // persisted (env-sourced) values.
        let is_env_sourced = existing.config_source == "env";

        // Endpoints from an issuer. Done here rather than in the caller
        // because it needs the row to know the provider's kind, and done with
        // `ProviderKind` rather than a second table of URL shapes so the
        // operator route and the `OAUTH_*` env route
        // (`oauth_env::resolve`) derive identically.
        let kind = ProviderKind::from_provider_key(&existing.provider_key);
        let derives_from_issuer = kind.is_some_and(|k| k.required_issuer_field().is_some());
        let supplied_issuer = if is_env_sourced {
            None
        } else {
            update
                .issuer_url
                .as_deref()
                .map(str::trim)
                .filter(|url| !url.is_empty())
        };
        if supplied_issuer.is_some() && !derives_from_issuer {
            return Ok(Err(format!(
                "{} publishes its own endpoints, so it takes no issuer URL.",
                existing.display_name
            )));
        }
        let issuer_url = supplied_issuer
            .map(str::to_string)
            .or_else(|| existing.issuer_url.clone());
        // Whenever an issuer is known the endpoints follow it, including on an
        // update that only changed the issuer: correcting a realm must move
        // all three, not leave two pointed at the old one.
        let derived = match (kind, issuer_url.as_deref()) {
            (Some(kind), Some(issuer)) if derives_from_issuer => {
                Some(kind.derive_endpoints(issuer))
            }
            _ => None,
        };
        let authorization_url = match &derived {
            Some(d) if !is_env_sourced => d.authorization_url.clone(),
            _ => existing.authorization_url.clone(),
        };
        let token_url = match &derived {
            Some(d) if !is_env_sourced => d.token_url.clone(),
            _ => existing.token_url.clone(),
        };

        let display_name = if is_env_sourced {
            existing.display_name
        } else {
            update.display_name.unwrap_or(existing.display_name)
        };
        let oauth_client_id = if is_env_sourced {
            existing.oauth_client_id
        } else {
            update.oauth_client_id.or(existing.oauth_client_id)
        };
        let oauth_client_secret = if is_env_sourced {
            existing.oauth_client_secret
        } else {
            update.oauth_client_secret.or(existing.oauth_client_secret)
        };
        let enabled = update.enabled.unwrap_or(existing.enabled);
        let userinfo_url = if is_env_sourced {
            existing.userinfo_url
        } else if let Some(derived) = &derived {
            // Derivation wins over a hand-typed userinfo URL here, because a
            // derived provider's form does not offer one: a userinfo endpoint
            // that disagrees with the issuer its tokens came from is not a
            // configuration anybody wants.
            derived.userinfo_url.clone()
        } else {
            update.userinfo_url.or(existing.userinfo_url)
        };
        let scopes = if is_env_sourced {
            existing.scopes
        } else {
            update
                .scopes
                .map(|items| items.into_iter().map(Some).collect::<Vec<_>>())
                .unwrap_or(existing.scopes)
        };
        // Credentials are not enough for an issuer-derived provider: the
        // seeded Keycloak row carries empty endpoint strings until an issuer
        // arrives, and calling that "configured" puts a sign-in button on the
        // login page that leads to `/authorize` on nothing.
        let configured = oauth_client_id.is_some()
            && oauth_client_secret.is_some()
            && !authorization_url.trim().is_empty()
            && !token_url.trim().is_empty();

        diesel::update(oauth_providers::table.filter(oauth_providers::id.eq(provider_id)))
            .set((
                oauth_providers::display_name.eq(display_name),
                oauth_providers::authorization_url.eq(authorization_url),
                oauth_providers::token_url.eq(token_url),
                oauth_providers::issuer_url.eq(issuer_url),
                oauth_providers::oauth_client_id.eq(oauth_client_id),
                oauth_providers::oauth_client_secret.eq(oauth_client_secret),
                oauth_providers::enabled.eq(enabled),
                oauth_providers::userinfo_url.eq(userinfo_url),
                oauth_providers::scopes.eq(scopes),
                oauth_providers::configured.eq(configured),
                oauth_providers::updated_at.eq(now),
            ))
            .execute(&mut conn)?;

        oauth_providers::table
            .filter(oauth_providers::id.eq(provider_id))
            .select(OAuthProvider::as_select())
            .first::<OAuthProvider>(&mut conn)
            .optional()
            .map(Ok)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to update OAuth provider".to_string())?
    .and_then(|row| row.ok_or_else(|| "OAuth provider not found".to_string()))
}

/// Startup-time materialization of `OAUTH_*` env-var-configured provider
/// instances into `oauth_providers` rows (ADR-041, research.md §3/§6).
///
/// - Every env-var-detected instance is upserted with `config_source =
///   "env"`; every writable field is refreshed on each run except `enabled`,
///   which is only set `true` on first insert — an admin's later toggle
///   must survive a restart.
/// - Any row that *was* `config_source = "env"` but whose env vars are no
///   longer present in this scan is flipped back to `config_source =
///   "admin"`, values untouched (never deleted — see research.md §6 on why
///   deleting would cascade-orphan linked `user_oauth_accounts`).
/// - Incomplete env-var groups are logged (FR-010) and skipped, never
///   panicking startup.
pub async fn materialize_env_oauth_providers(db_pool: &DbPool) -> Result<(), String> {
    let parsed = parse_oauth_env_vars(std::env::vars());
    let mut resolved = Vec::with_capacity(parsed.len());
    for instance in &parsed {
        match resolve(instance) {
            Ok(r) => resolved.push(r),
            Err(missing) => {
                tracing::warn!(
                    provider = %missing.provider,
                    instance = %missing.instance,
                    missing_field = %missing.field,
                    "OAuth env-var provider instance is missing a required setting; skipping"
                );
            }
        }
    }

    let mut conn = db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        let now = Utc::now().naive_utc();
        let mut seen_keys: HashSet<String> = HashSet::new();

        for r in &resolved {
            seen_keys.insert(r.provider_key.clone());
            let scopes: Vec<Option<String>> = r.scopes.iter().cloned().map(Some).collect();

            let existing = oauth_providers::table
                .filter(oauth_providers::provider_key.eq(&r.provider_key))
                .select(OAuthProvider::as_select())
                .first::<OAuthProvider>(&mut conn)
                .optional()?;

            match existing {
                Some(row) => {
                    // Only set enabled=true on the row's first transition
                    // into config_source="env"; an already-env-sourced row
                    // keeps whatever an admin last toggled it to.
                    let enabled = if row.config_source == "env" {
                        row.enabled
                    } else {
                        true
                    };
                    diesel::update(oauth_providers::table.filter(oauth_providers::id.eq(row.id)))
                        .set((
                            oauth_providers::display_name.eq(&r.display_name),
                            oauth_providers::authorization_url.eq(&r.authorization_url),
                            oauth_providers::token_url.eq(&r.token_url),
                            oauth_providers::userinfo_url.eq(&r.userinfo_url),
                            oauth_providers::issuer_url.eq(&r.issuer_url),
                            oauth_providers::scopes.eq(&scopes),
                            oauth_providers::oauth_client_id.eq(Some(&r.client_id)),
                            oauth_providers::oauth_client_secret.eq(Some(&r.client_secret)),
                            oauth_providers::configured.eq(true),
                            oauth_providers::config_source.eq("env"),
                            oauth_providers::enabled.eq(enabled),
                            oauth_providers::updated_at.eq(now),
                        ))
                        .execute(&mut conn)?;
                }
                None => {
                    let new_row = NewOAuthProvider {
                        id: uuid::Uuid::now_v7(),
                        provider_key: r.provider_key.clone(),
                        display_name: r.display_name.clone(),
                        authorization_url: r.authorization_url.clone(),
                        token_url: r.token_url.clone(),
                        userinfo_url: r.userinfo_url.clone(),
                        scopes,
                        oauth_client_id: Some(r.client_id.clone()),
                        oauth_client_secret: Some(r.client_secret.clone()),
                        configured: true,
                        enabled: true,
                        created_at: now,
                        updated_at: now,
                        config_source: "env".to_string(),
                        issuer_url: r.issuer_url.clone(),
                    };
                    diesel::insert_into(oauth_providers::table)
                        .values(&new_row)
                        .execute(&mut conn)?;
                }
            }
        }

        // Anything still config_source="env" but not seen in this scan had
        // its env vars removed — revert to admin-editable, values retained.
        let stale_env_rows = oauth_providers::table
            .filter(oauth_providers::config_source.eq("env"))
            .select(OAuthProvider::as_select())
            .load::<OAuthProvider>(&mut conn)?;
        for row in stale_env_rows {
            if !seen_keys.contains(&row.provider_key) {
                diesel::update(oauth_providers::table.filter(oauth_providers::id.eq(row.id)))
                    .set((
                        oauth_providers::config_source.eq("admin"),
                        oauth_providers::updated_at.eq(now),
                    ))
                    .execute(&mut conn)?;
            }
        }

        Ok::<_, diesel::result::Error>(())
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to materialize env-configured OAuth providers".to_string())
}
