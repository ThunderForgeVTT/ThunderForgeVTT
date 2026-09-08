use crate::auth_middleware::resolve_authenticated_user;
use crate::models::{
    AdminBootstrapOAuthSession, AdminBootstrapSetup, AuthSecuritySetting, LoginTwoFactorChallenge,
    NewAdminBootstrapOAuthSession, NewAdminBootstrapSetup, NewLoginTwoFactorChallenge,
    NewOAuthAuthorizationSession, NewOAuthLinkChallenge, NewUserOAuthAccount, NewUserSession,
    OAuthAuthorizationSession, OAuthLinkChallenge, OAuthProvider, UserOAuthAccount,
};
use crate::schema::{
    admin_bootstrap_oauth_sessions, admin_bootstrap_setup, auth_security_settings,
    login_two_factor_challenges, oauth_authorization_sessions, oauth_link_challenges,
    oauth_providers, user_oauth_accounts, user_sessions, users,
};
use crate::state::AppState;
use crate::users::{PublicUser, load_public_user, record_auth_audit_event};
// Moved to `crate::crypto` so spec 034's repository credentials can use the
// same implementation rather than a second one. See that module's header.
use crate::crypto::{decrypt_secret, encrypt_secret, encryption_key_from_config_secret};
use argon2::password_hash::PasswordHasher;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect};
use axum::{
    Json, Router,
    routing::{get, post},
};
use chrono::Utc;
use data_encoding::BASE32_NOPAD;
use diesel::prelude::*;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use thunderforge_axum_auth_core::random::random_urlsafe;
use thunderforge_axum_auth_core::session::{self, CookieSpec, csrf_cookie, session_cookie};
use thunderforge_axum_auth_core::totp::verify_totp_code;
use thunderforge_axum_oauth::authorize::{AuthorizeRequest, build_authorize_url};
use thunderforge_axum_oauth::error::provider_error_from_callback;
use thunderforge_axum_oauth::pkce::{code_challenge_from_verifier, generate_code_verifier};
use thunderforge_axum_oauth::state::generate_state;
// The token endpoint's reply, including the `id_token` this struct used to
// drop on the floor — see `extract_provider_user_id_from_token`.
use thunderforge_axum_oauth::token::TokenResponse as OAuthTokenResponse;
use thunderforge_axum_oidc::id_token::subject_from_id_token_unverified;
use thunderforge_axum_oidc::userinfo::{
    extract_email as extract_provider_email, extract_subject as extract_provider_user_id,
};
use thunderforge_core::auth::Credentials;
use tower_cookies::cookie::SameSite;
use tower_cookies::{Cookie, Cookies};
use url::Url;

/// Spec 002: `require_world_member` — the shared world_members-based
/// authorization guard for canvas asset reads/writes.
/// Spec 035 / ADR-072: the instance admission policy and its audit trail.
pub mod instance_access;
pub(crate) use instance_access::{AdmissionRoute, record_refusal};
pub mod world_membership;

/// Spec 028 (T045c): `scenes.hidden` visibility for scenes and the canvas
/// assets attached to them — the rule the sync plan and the byte route must
/// answer identically.
pub mod scene_visibility;

/// Spec 010: actor ownership/permission enforcement (`require_actor_permission`,
/// `is_dm_of_world`).
/// Spec 027 (US5): the single declaration of every permissioned content type,
/// generating permission resolution and member-removal cleanup for all of
/// them. The four `*_permissions` modules below re-export from here.
pub mod permissioned_entities;

/// Spec 031 (FR-044, FR-045): which authoring tools a person may use. A
/// sibling of `permissioned_entities` rather than an entry in it — the module
/// says why.
pub mod authoring_tools;

pub mod actor_permissions;

/// Spec 012: lore entry ownership/permission enforcement
/// (`require_lore_permission`, `effective_lore_permission`) — generalizes
/// `actor_permissions` to `world_lore_entries`.
pub mod lore_permissions;

/// Spec 013: item ownership/permission enforcement (`require_item_permission`),
/// a direct structural mirror of `actor_permissions`.
pub mod ability_permissions;
pub mod item_permissions;

/// Registration/bootstrap identity concerns (input validation, registration
/// gating, username derivation for manual + OAuth-auto-provisioned
/// accounts) split out of this module for focused unit testing.
mod registration;

/// The provider-wiring guarantee: every `ProviderKind` we declare is walked
/// from env var to live authorization redirect. Test-only — read its module
/// documentation for what it catches and why a crate split alone does not.
#[cfg(test)]
mod provider_wiring;

use registration::{
    Admission, AdmissionRefused, RegisterUserError, derive_bootstrap_username,
    ensure_admission_allowed, random_setup_code, unique_username_from_email_sync,
    validate_registration_input,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/authentication/setup/status", get(setup_status))
        .route("/authentication/setup/basic", post(admin_setup_basic))
        // Spec 040 US1: one step of the pass, written as it is completed
        // (FR-006), and the one place setup finishes (FR-002a).
        .route("/authentication/setup/settings", post(setup_settings))
        .route("/authentication/setup/complete", post(setup_complete))
        .route(
            "/authentication/setup/oauth/{provider_key}/start",
            post(admin_setup_oauth_start),
        )
        .route(
            "/authentication/setup/oauth/{provider_key}/callback",
            get(admin_setup_oauth_callback),
        )
        .route("/authentication/basic", post(basic_authentication))
        .route("/authentication/login", post(login))
        .route("/authentication/register", post(register))
        .route("/authentication/session", get(current_session))
        .route("/authentication/session/refresh", post(refresh_session))
        .route("/authentication/oauth/resolve", post(oauth_resolve))
        .route(
            "/authentication/oauth/link/confirm",
            post(oauth_link_confirm),
        )
        .route(
            "/authentication/oauth/{provider_key}/start",
            get(oauth_start),
        )
        .route(
            "/authentication/oauth/{provider_key}/callback",
            get(oauth_callback),
        )
        .route(
            "/authentication/oauth/{provider_key}/token",
            post(oauth_token_exchange),
        )
        .route(
            "/authentication/2fa/setup/start",
            post(two_factor_setup_start),
        )
        .route(
            "/authentication/2fa/setup/confirm",
            post(two_factor_setup_confirm),
        )
        .route("/authentication/2fa/verify", post(two_factor_verify))
        // Spec 041 FR-005: what this account's own second factor looks like.
        // The enrolment screen cannot offer to turn something on without
        // being able to ask whether it already is.
        .route("/authentication/2fa/status", get(two_factor_status))
        // Spec 041 US2 (FR-010): a fresh set of recovery codes, which costs
        // possession of the factor and invalidates every earlier code.
        .route(
            "/authentication/2fa/recovery-codes",
            post(regenerate_recovery_codes),
        )
        .route(
            "/authentication/admin/2fa/requirement",
            post(set_admin_two_factor_requirement),
        )
        .route(
            "/authentication/admin/users/{user_id}/2fa/required",
            post(set_admin_user_two_factor_required),
        )
        .route("/authentication/logout", post(logout))
}

#[path = "types.rs"]
pub(crate) mod types;
pub(crate) use types::*;

/// Spec 040 US1: what setup still needs and whether it may finish. Separate
/// from the handlers so the predicate can be read and tested without an HTTP
/// request in sight, and so `admin_setup.rs` stays under the 1000-line gate.
#[path = "setup_requirements.rs"]
pub(crate) mod setup_requirements;

#[path = "admin_setup.rs"]
pub(crate) mod admin_setup;
pub(crate) use admin_setup::*;

#[path = "sessions.rs"]
pub(crate) mod sessions;
pub(crate) use sessions::*;

/// Spec 036 US4: reading and ending the sessions one account holds. Separate
/// from `sessions`, which is about becoming signed in.
#[path = "session_registry.rs"]
pub mod session_registry;

#[path = "two_factor.rs"]
pub(crate) mod two_factor;
pub(crate) use two_factor::*;

#[path = "oauth.rs"]
pub(crate) mod oauth;
pub(crate) use oauth::*;

#[path = "admin_bootstrap.rs"]
pub mod admin_bootstrap;
pub use admin_bootstrap::*;

fn error_response(
    code: StatusCode,
    status: &'static str,
    message: &str,
) -> (StatusCode, Json<OAuthResponse>) {
    (
        code,
        Json(OAuthResponse {
            status,
            message: message.to_string(),
            challenge_id: None,
            login_two_factor_challenge_id: None,
        }),
    )
}

#[cfg(test)]
#[path = "argon2_upgrade_tests.rs"]
mod argon2_upgrade_tests;
