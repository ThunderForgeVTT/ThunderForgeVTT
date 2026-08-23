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
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::password_hash::{PasswordHasher, SaltString, rand_core::OsRng};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect};
use axum::{
    Json, Router,
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose};
use chrono::Utc;
use data_encoding::BASE32_NOPAD;
use diesel::prelude::*;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thunderforge_core::auth::Credentials;
use totp_rs::{Algorithm, TOTP};
use tower_cookies::cookie::SameSite;
use tower_cookies::{Cookie, Cookies};
use url::Url;

/// Spec 002: `require_world_member` — the shared world_members-based
/// authorization guard for canvas asset reads/writes.
pub mod world_membership;

/// Spec 010: actor ownership/permission enforcement (`require_actor_permission`,
/// `is_dm_of_world`).
pub mod actor_permissions;

/// Spec 013: item ownership/permission enforcement (`require_item_permission`),
/// a direct structural mirror of `actor_permissions`.
pub mod item_permissions;

/// Registration/bootstrap identity concerns (input validation, registration
/// gating, username derivation for manual + OAuth-auto-provisioned
/// accounts) split out of this module for focused unit testing.
mod registration;

use registration::{
    RegisterUserError, derive_bootstrap_username, ensure_registration_allowed, random_setup_code,
    unique_username_from_email_sync, validate_registration_input,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/authentication/setup/status", get(setup_status))
        .route("/authentication/setup/basic", post(admin_setup_basic))
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

#[derive(Debug, Deserialize)]
struct LoginRequest {
    identifier: String,
    password: String,
    two_factor_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    username: String,
    email: String,
    password: String,
}

#[derive(Debug, Deserialize, Clone)]
struct OAuthResolveRequest {
    provider_key: String,
    provider_user_id: String,
    provider_email: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
    token_expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
struct OAuthLinkConfirmRequest {
    challenge_id: uuid::Uuid,
    password: String,
}

#[derive(Debug, Deserialize)]
struct TwoFactorSetupStartRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct TwoFactorSetupStartResponse {
    status: &'static str,
    message: String,
    otpauth_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TwoFactorSetupConfirmRequest {
    username: String,
    password: String,
    code: String,
}

#[derive(Debug, Deserialize)]
struct TwoFactorVerifyRequest {
    challenge_id: uuid::Uuid,
    code: String,
}

#[derive(Debug, Deserialize)]
struct AdminTwoFactorRequirementRequest {
    required_for_all_users: bool,
}

#[derive(Debug, Deserialize)]
struct AdminUserTwoFactorRequiredRequest {
    required: bool,
}

#[derive(Debug, Deserialize)]
struct OAuthStartQuery {
    redirect_uri: String,
    return_to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OAuthCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenExchangeRequest {
    code: String,
    state: String,
}

#[derive(Debug, Serialize)]
struct SetupStatusResponse {
    setup_required: bool,
    setup_completed: bool,
    configured_oauth_providers: Vec<SetupOAuthProvider>,
}

#[derive(Debug, Serialize)]
struct SetupOAuthProvider {
    provider_key: String,
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct AdminSetupBasicRequest {
    admin_code: String,
    username: String,
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct AdminSetupOAuthStartRequest {
    admin_code: String,
    redirect_uri: String,
    username: Option<String>,
    return_to: Option<String>,
}

#[derive(Debug, Serialize)]
struct AdminSetupOAuthStartResponse {
    authorization_url: String,
}

#[derive(Debug, Serialize)]
struct OAuthResponse {
    status: &'static str,
    message: String,
    challenge_id: Option<uuid::Uuid>,
    login_two_factor_challenge_id: Option<uuid::Uuid>,
}

#[derive(Debug, Serialize)]
struct SessionStateResponse {
    authenticated: bool,
    user: PublicUser,
    session_expires_at: chrono::NaiveDateTime,
}

#[derive(Debug, Serialize)]
struct AuthSessionResponse {
    status: &'static str,
    message: String,
    session: Option<SessionStateResponse>,
    login_two_factor_challenge_id: Option<uuid::Uuid>,
    requires_email_verification: bool,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

struct OAuthAuthorizationContext {
    provider: OAuthProvider,
    session: OAuthAuthorizationSession,
}

struct AdminBootstrapOAuthContext {
    provider: OAuthProvider,
    session: AdminBootstrapOAuthSession,
}

enum ResolveOutcome {
    ProviderNotFound,
    LinkedUser(uuid::Uuid),
    PasswordRequired(uuid::Uuid),
    NoMatchingUser,
}

enum LinkConfirmOutcome {
    ChallengeInvalid,
    ChallengeExpired,
    PasswordMismatch,
    LinkConflict,
    Linked(uuid::Uuid),
}

async fn setup_status(State(state): State<AppState>) -> (StatusCode, Json<SetupStatusResponse>) {
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");

    let result = tokio::task::spawn_blocking(move || {
        let admin_exists = users::table
            .filter(users::is_admin.eq(true))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()?;

        let setup = admin_bootstrap_setup::table
            .filter(admin_bootstrap_setup::id.eq(1))
            .select(AdminBootstrapSetup::as_select())
            .first::<AdminBootstrapSetup>(&mut conn)
            .optional()?;

        let providers = oauth_providers::table
            .filter(oauth_providers::enabled.eq(true))
            .filter(oauth_providers::configured.eq(true))
            .select((oauth_providers::provider_key, oauth_providers::display_name))
            .load::<(String, String)>(&mut conn)?;

        Ok::<_, diesel::result::Error>((admin_exists.is_some(), setup, providers))
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query setup status");

    let (admin_exists, setup, providers) = result;
    let setup_completed = admin_exists || setup.and_then(|v| v.setup_completed_at).is_some();

    (
        StatusCode::OK,
        Json(SetupStatusResponse {
            setup_required: !setup_completed,
            setup_completed,
            configured_oauth_providers: providers
                .into_iter()
                .map(|(provider_key, display_name)| SetupOAuthProvider {
                    provider_key,
                    display_name,
                })
                .collect(),
        }),
    )
}

async fn admin_setup_basic(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<AdminSetupBasicRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    if let Err(resp) = ensure_admin_setup_code_valid(&state, &request.admin_code).await {
        return resp;
    }

    let username = request.username.trim().to_string();
    let email = request.email.trim().to_lowercase();
    if username.is_empty() || email.is_empty() || request.password.is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Username, email, and password are required",
        );
    }

    let password_hash = match hash_password(&request.password) {
        Ok(v) => v,
        Err(msg) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "password_hash_failed",
                msg.as_str(),
            );
        }
    };

    let now = Utc::now().naive_utc();
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let result = tokio::task::spawn_blocking(move || -> Result<uuid::Uuid, String> {
        let admin_exists = users::table
            .filter(users::is_admin.eq(true))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to query existing admins".to_string())?;
        if admin_exists.is_some() {
            return Err("Setup has already been completed".to_string());
        }

        let username_exists = users::table
            .filter(users::username.eq(&username))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to validate username".to_string())?;
        if username_exists.is_some() {
            return Err("Username is already in use".to_string());
        }

        let email_exists = users::table
            .filter(users::email.eq(&email))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to validate email".to_string())?;
        if email_exists.is_some() {
            return Err("Email is already in use".to_string());
        }

        let user_id = uuid::Uuid::now_v7();
        diesel::insert_into(users::table)
            .values((
                users::id.eq(user_id),
                users::username.eq(username),
                users::email.eq(email),
                users::is_admin.eq(true),
                users::password_hash.eq(password_hash),
                users::created_at.eq(now),
                users::updated_at.eq(now),
                users::two_factor_enabled.eq(false),
                users::two_factor_secret_encrypted.eq::<Option<String>>(None),
                users::two_factor_confirmed_at.eq::<Option<chrono::NaiveDateTime>>(None),
                users::two_factor_admin_required.eq(false),
            ))
            .execute(&mut conn)
            .map_err(|_| "Failed to create admin user".to_string())?;

        mark_admin_setup_complete_sync(&mut conn, now)?;

        Ok(user_id)
    })
    .await
    .expect("Failed to spawn blocking task");

    let user_id = match result {
        Ok(v) => v,
        Err(msg) if msg == "Setup has already been completed" => {
            return error_response(StatusCode::CONFLICT, "setup_complete", msg.as_str());
        }
        Err(msg) if msg == "Username is already in use" || msg == "Email is already in use" => {
            return error_response(StatusCode::CONFLICT, "setup_conflict", msg.as_str());
        }
        Err(msg) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "setup_error",
                msg.as_str(),
            );
        }
    };

    if let Err(msg) = issue_session_cookie(&state, &cookies, user_id).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session_error",
            msg.as_str(),
        );
    }

    (
        StatusCode::OK,
        Json(OAuthResponse {
            status: "success",
            message: "Initial admin account created successfully".to_string(),
            challenge_id: None,
            login_two_factor_challenge_id: None,
        }),
    )
}

async fn admin_setup_oauth_start(
    Path(provider_key): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<AdminSetupOAuthStartRequest>,
) -> Result<(StatusCode, Json<AdminSetupOAuthStartResponse>), (StatusCode, Json<OAuthResponse>)> {
    ensure_admin_setup_code_valid(&state, &request.admin_code).await?;

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let provider_key_clone = provider_key.clone();
    let now = Utc::now().naive_utc();
    let state_token = random_urlsafe(32);
    let code_verifier = random_urlsafe(48);

    let provider = tokio::task::spawn_blocking(move || {
        oauth_providers::table
            .filter(oauth_providers::provider_key.eq(provider_key_clone))
            .filter(oauth_providers::enabled.eq(true))
            .filter(oauth_providers::configured.eq(true))
            .select(OAuthProvider::as_select())
            .first::<OAuthProvider>(&mut conn)
            .optional()
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query DB")
    .ok_or_else(|| {
        error_response(
            StatusCode::NOT_FOUND,
            "provider_not_found",
            "OAuth provider is not configured or disabled",
        )
    })?;

    let Some(provider_client_id) = provider.oauth_client_id.clone() else {
        return Err(error_response(
            StatusCode::CONFLICT,
            "provider_not_configured",
            "Provider client id is not set",
        ));
    };

    let session = NewAdminBootstrapOAuthSession {
        id: uuid::Uuid::now_v7(),
        provider_id: provider.id,
        oauth_provider_key: provider_key.clone(),
        oauth_client_id: provider_client_id.clone(),
        state: state_token.clone(),
        code_verifier: code_verifier.clone(),
        redirect_uri: request.redirect_uri.clone(),
        desired_username: request.username,
        return_to: request.return_to,
        expires_at: now + chrono::Duration::minutes(10),
        consumed_at: None,
        created_at: now,
    };

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    tokio::task::spawn_blocking(move || {
        diesel::insert_into(admin_bootstrap_oauth_sessions::table)
            .values(&session)
            .execute(&mut conn)
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to persist bootstrap oauth session");

    let code_challenge = code_challenge_from_verifier(&code_verifier);
    let mut url = Url::parse(&provider.authorization_url).map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "provider_misconfigured",
            "Provider authorization URL is invalid",
        )
    })?;

    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &provider_client_id)
        .append_pair("redirect_uri", &request.redirect_uri)
        .append_pair(
            "scope",
            &provider
                .scopes
                .iter()
                .filter_map(|s| s.as_ref())
                .cloned()
                .collect::<Vec<String>>()
                .join(" "),
        )
        .append_pair("state", &state_token)
        .append_pair("code_challenge", &code_challenge)
        .append_pair("code_challenge_method", "S256");

    Ok((
        StatusCode::OK,
        Json(AdminSetupOAuthStartResponse {
            authorization_url: url.to_string(),
        }),
    ))
}

async fn admin_setup_oauth_callback(
    Path(provider_key): Path<String>,
    Query(query): Query<OAuthCallbackQuery>,
    cookies: Cookies,
    State(state): State<AppState>,
) -> axum::response::Response {
    if let Some(err) = query.error {
        let message = format!(
            "Provider returned error '{}': {}",
            err,
            query
                .error_description
                .unwrap_or_else(|| "unknown".to_string())
        );
        return bootstrap_error_redirect(&message);
    }

    let Some(code) = query.code else {
        return bootstrap_error_redirect("Missing 'code' query parameter");
    };
    let Some(state_token) = query.state else {
        return bootstrap_error_redirect("Missing 'state' query parameter");
    };

    let auth_ctx =
        match load_and_consume_admin_bootstrap_oauth_session(&state, &provider_key, &state_token)
            .await
        {
            Ok(v) => v,
            Err(_) => {
                return bootstrap_error_redirect("Bootstrap OAuth state is invalid or expired");
            }
        };

    let token_response = match exchange_authorization_code_with_provider(
        &auth_ctx.provider,
        &auth_ctx.session.redirect_uri,
        &auth_ctx.session.code_verifier,
        &code,
    )
    .await
    {
        Ok(tokens) => tokens,
        Err(msg) => {
            return bootstrap_error_redirect(msg.as_str());
        }
    };

    let userinfo = if let Some(userinfo_url) = auth_ctx.provider.userinfo_url.clone() {
        match fetch_userinfo(userinfo_url, token_response.access_token.clone()).await {
            Ok(v) => Some(v),
            Err(msg) => {
                return bootstrap_error_redirect(msg.as_str());
            }
        }
    } else {
        None
    };

    let provider_user_id = userinfo
        .as_ref()
        .and_then(extract_provider_user_id)
        .or_else(|| extract_provider_user_id_from_token(&token_response));

    let Some(provider_user_id) = provider_user_id else {
        return bootstrap_error_redirect(
            "Could not extract provider user id from provider response",
        );
    };

    let provider_email = userinfo.as_ref().and_then(extract_provider_email);
    let desired_username = auth_ctx.session.desired_username.clone();
    let return_to = auth_ctx.session.return_to.clone();
    let user_id = match create_admin_user_from_oauth(
        &state,
        auth_ctx.provider.id,
        provider_user_id,
        provider_email,
        desired_username,
        token_response,
    )
    .await
    {
        Ok(v) => v,
        Err((_, payload)) => return bootstrap_error_redirect(payload.message.as_str()),
    };

    if let Err(msg) = issue_session_cookie(&state, &cookies, user_id).await {
        return bootstrap_error_redirect(msg.as_str());
    }

    if let Some(return_to) = return_to
        && let Ok(url) = Url::parse(&return_to)
    {
        return Redirect::temporary(url.as_str()).into_response();
    }

    (
        StatusCode::OK,
        Json(OAuthResponse {
            status: "success",
            message: "Initial admin account created successfully via OAuth".to_string(),
            challenge_id: None,
            login_two_factor_challenge_id: None,
        }),
    )
        .into_response()
}

fn bootstrap_error_redirect(message: &str) -> axum::response::Response {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("oauth_error", message);
    let query = serializer.finish();
    let target = format!("/setup/callback?{query}");
    Redirect::temporary(target.as_str()).into_response()
}

async fn basic_authentication(
    cookies: Cookies,
    headers: HeaderMap,
    State(state): State<AppState>,
    credentials: String,
) -> (StatusCode, Json<OAuthResponse>) {
    let cred = match Credentials::decode(&credentials) {
        Ok(value) => value,
        Err(message) => {
            return error_response(StatusCode::BAD_REQUEST, "invalid_request", message.as_str());
        }
    };

    let code = headers
        .get("x-2fa-code")
        .and_then(|h| h.to_str().ok())
        .map(|v| v.to_string());

    let (status, response) = authenticate_password_login(
        &state,
        &cookies,
        &cred.username,
        &cred.password,
        code.as_deref(),
    )
    .await;

    (
        status,
        Json(OAuthResponse {
            status: response.status,
            message: response.message.clone(),
            challenge_id: None,
            login_two_factor_challenge_id: response.login_two_factor_challenge_id,
        }),
    )
}

async fn login(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> (StatusCode, Json<AuthSessionResponse>) {
    authenticate_password_login(
        &state,
        &cookies,
        &request.identifier,
        &request.password,
        request.two_factor_code.as_deref(),
    )
    .await
}

async fn register(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> (StatusCode, Json<AuthSessionResponse>) {
    if let Err(message) = ensure_registration_allowed(&state).await {
        return auth_session_error(
            StatusCode::CONFLICT,
            "registration_blocked",
            message.as_str(),
        );
    }

    let username = request.username.trim().to_string();
    let email = request.email.trim().to_lowercase();

    if let Err(message) = validate_registration_input(&username, &email, &request.password) {
        return auth_session_error(StatusCode::BAD_REQUEST, "invalid_request", message.as_str());
    }

    let password_hash = match hash_password(&request.password) {
        Ok(value) => value,
        Err(message) => {
            return auth_session_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "password_hash_failed",
                message.as_str(),
            );
        }
    };

    let now = Utc::now().naive_utc();
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let create_result =
        tokio::task::spawn_blocking(move || -> Result<uuid::Uuid, RegisterUserError> {
            let username_exists = users::table
                .filter(users::username.eq(&username))
                .select(users::id)
                .first::<uuid::Uuid>(&mut conn)
                .optional()
                .map_err(|_| RegisterUserError::Storage)?;
            if username_exists.is_some() {
                return Err(RegisterUserError::UsernameTaken);
            }

            let email_exists = users::table
                .filter(users::email.eq(&email))
                .select(users::id)
                .first::<uuid::Uuid>(&mut conn)
                .optional()
                .map_err(|_| RegisterUserError::Storage)?;
            if email_exists.is_some() {
                return Err(RegisterUserError::EmailTaken);
            }

            let user_id = uuid::Uuid::now_v7();
            diesel::insert_into(users::table)
                .values((
                    users::id.eq(user_id),
                    users::username.eq(username),
                    users::email.eq(email),
                    users::is_admin.eq(false),
                    users::password_hash.eq(password_hash),
                    users::created_at.eq(now),
                    users::updated_at.eq(now),
                    users::two_factor_enabled.eq(false),
                    users::two_factor_secret_encrypted.eq::<Option<String>>(None),
                    users::two_factor_confirmed_at.eq::<Option<chrono::NaiveDateTime>>(None),
                    users::two_factor_admin_required.eq(false),
                ))
                .execute(&mut conn)
                .map_err(|_| RegisterUserError::Storage)?;

            Ok(user_id)
        })
        .await
        .expect("Failed to spawn blocking task");

    let user_id = match create_result {
        Ok(value) => value,
        Err(RegisterUserError::UsernameTaken) => {
            return auth_session_error(
                StatusCode::CONFLICT,
                "username_taken",
                "Username is already in use",
            );
        }
        Err(RegisterUserError::EmailTaken) => {
            return auth_session_error(
                StatusCode::CONFLICT,
                "email_taken",
                "Email is already in use",
            );
        }
        Err(RegisterUserError::Storage) => {
            return auth_session_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "registration_failed",
                "Failed to create account",
            );
        }
    };

    let session = match issue_session_cookie(&state, &cookies, user_id).await {
        Ok(value) => value,
        Err(message) => {
            return auth_session_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "session_error",
                message.as_str(),
            );
        }
    };

    match build_session_response(
        &state,
        user_id,
        session.expires_at,
        "success",
        "Account created successfully",
    )
    .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)),
        Err(message) => auth_session_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session_error",
            message.as_str(),
        ),
    }
}

async fn current_session(
    cookies: Cookies,
    State(state): State<AppState>,
) -> (StatusCode, Json<AuthSessionResponse>) {
    let authenticated_user = match resolve_authenticated_user(&state, &cookies).await {
        Ok(value) => value,
        Err(StatusCode::UNAUTHORIZED) => {
            return auth_session_error(
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "No active session",
            );
        }
        Err(_) => {
            return auth_session_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "session_error",
                "Failed to validate current session",
            );
        }
    };

    match build_session_response(
        &state,
        authenticated_user.user_id,
        authenticated_user.expires_at,
        "authenticated",
        "Active session found",
    )
    .await
    {
        Ok(response) => (StatusCode::OK, Json(response)),
        Err(message) => auth_session_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session_error",
            message.as_str(),
        ),
    }
}

async fn refresh_session(
    cookies: Cookies,
    State(state): State<AppState>,
) -> (StatusCode, Json<AuthSessionResponse>) {
    let authenticated_user = match resolve_authenticated_user(&state, &cookies).await {
        Ok(value) => value,
        Err(StatusCode::UNAUTHORIZED) => {
            return auth_session_error(
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "No active session to refresh",
            );
        }
        Err(_) => {
            return auth_session_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "session_error",
                "Failed to validate current session",
            );
        }
    };

    let session = match issue_session_cookie(&state, &cookies, authenticated_user.user_id).await {
        Ok(value) => value,
        Err(message) => {
            return auth_session_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "session_error",
                message.as_str(),
            );
        }
    };

    match build_session_response(
        &state,
        authenticated_user.user_id,
        session.expires_at,
        "refreshed",
        "Session rotated successfully",
    )
    .await
    {
        Ok(response) => (StatusCode::OK, Json(response)),
        Err(message) => auth_session_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session_error",
            message.as_str(),
        ),
    }
}

async fn two_factor_setup_start(
    State(state): State<AppState>,
    Json(request): Json<TwoFactorSetupStartRequest>,
) -> (StatusCode, Json<TwoFactorSetupStartResponse>) {
    let username = request.username.clone();
    let username_for_query = username.clone();
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");

    let user = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::username.eq(&username_for_query))
            .select((users::id, users::password_hash))
            .first::<(uuid::Uuid, String)>(&mut conn)
            .optional()
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query DB");

    let Some((user_id, password_hash)) = user else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(TwoFactorSetupStartResponse {
                status: "failure",
                message: "Invalid credentials".to_string(),
                otpauth_url: None,
            }),
        );
    };

    let parsed_hash = PasswordHash::new(&password_hash).expect("Invalid hash in db");
    if Argon2::default()
        .verify_password(request.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(TwoFactorSetupStartResponse {
                status: "failure",
                message: "Invalid credentials".to_string(),
                otpauth_url: None,
            }),
        );
    }

    let secret_base32 = {
        let mut secret_bytes = [0u8; 20];
        let mut rng = rand::rng();
        rng.fill(&mut secret_bytes);
        BASE32_NOPAD.encode(&secret_bytes)
    };

    let encryption_key = match encryption_key_from_config_secret(&state.config.secret) {
        Ok(key) => key,
        Err(msg) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TwoFactorSetupStartResponse {
                    status: "error",
                    message: msg,
                    otpauth_url: None,
                }),
            );
        }
    };

    let encrypted_secret = match encrypt_secret(&secret_base32, &encryption_key) {
        Ok(value) => value,
        Err(msg) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TwoFactorSetupStartResponse {
                    status: "error",
                    message: msg,
                    otpauth_url: None,
                }),
            );
        }
    };

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    tokio::task::spawn_blocking(move || {
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set((
                users::two_factor_secret_encrypted.eq(Some(encrypted_secret)),
                users::two_factor_enabled.eq(false),
                users::two_factor_confirmed_at.eq::<Option<chrono::NaiveDateTime>>(None),
            ))
            .execute(&mut conn)
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to update user 2FA secret");

    let otpauth = format!(
        "otpauth://totp/ThunderForge:{}?secret={}&issuer=ThunderForge",
        username, secret_base32
    );

    (
        StatusCode::OK,
        Json(TwoFactorSetupStartResponse {
            status: "success",
            message: "2FA secret generated. Confirm with one OTP code to enable.".to_string(),
            otpauth_url: Some(otpauth),
        }),
    )
}

async fn two_factor_setup_confirm(
    State(state): State<AppState>,
    Json(request): Json<TwoFactorSetupConfirmRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    let username = request.username.clone();
    let username_for_query = username.clone();
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");

    let user = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::username.eq(&username_for_query))
            .select((
                users::id,
                users::password_hash,
                users::two_factor_secret_encrypted,
            ))
            .first::<(uuid::Uuid, String, Option<String>)>(&mut conn)
            .optional()
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query DB");

    let Some((user_id, password_hash, secret_encrypted)) = user else {
        return error_response(StatusCode::UNAUTHORIZED, "failure", "Invalid credentials");
    };

    let parsed_hash = PasswordHash::new(&password_hash).expect("Invalid hash in db");
    if Argon2::default()
        .verify_password(request.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return error_response(StatusCode::UNAUTHORIZED, "failure", "Invalid credentials");
    }

    let Some(secret_encrypted) = secret_encrypted else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "two_factor_not_setup",
            "Start 2FA setup first",
        );
    };

    let encryption_key = match encryption_key_from_config_secret(&state.config.secret) {
        Ok(key) => key,
        Err(msg) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "two_factor_error",
                msg.as_str(),
            );
        }
    };

    let secret = match decrypt_secret(&secret_encrypted, &encryption_key) {
        Ok(value) => value,
        Err(msg) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "two_factor_error",
                msg.as_str(),
            );
        }
    };

    let now = Utc::now().naive_utc();
    match verify_totp_code(&username, &secret, &request.code) {
        Ok(true) => {
            let mut conn = state.db_pool.get().expect("Failed to get DB connection");
            tokio::task::spawn_blocking(move || {
                diesel::update(users::table.filter(users::id.eq(user_id)))
                    .set((
                        users::two_factor_enabled.eq(true),
                        users::two_factor_confirmed_at.eq(Some(now)),
                    ))
                    .execute(&mut conn)
            })
            .await
            .expect("Failed to spawn blocking task")
            .expect("Failed to enable 2FA");

            (
                StatusCode::OK,
                Json(OAuthResponse {
                    status: "success",
                    message: "2FA enabled".to_string(),
                    challenge_id: None,
                    login_two_factor_challenge_id: None,
                }),
            )
        }
        Ok(false) => error_response(
            StatusCode::UNAUTHORIZED,
            "two_factor_invalid",
            "Invalid 2FA code",
        ),
        Err(msg) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "two_factor_error",
            msg.as_str(),
        ),
    }
}

async fn two_factor_verify(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<TwoFactorVerifyRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    let now = Utc::now().naive_utc();
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");

    let challenge = tokio::task::spawn_blocking(move || {
        login_two_factor_challenges::table
            .filter(login_two_factor_challenges::id.eq(request.challenge_id))
            .select(LoginTwoFactorChallenge::as_select())
            .first::<LoginTwoFactorChallenge>(&mut conn)
            .optional()
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query 2FA challenge");

    let Some(challenge) = challenge else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "two_factor_challenge_invalid",
            "2FA challenge is invalid",
        );
    };

    if challenge.consumed_at.is_some() || challenge.expires_at <= now {
        return error_response(
            StatusCode::BAD_REQUEST,
            "two_factor_challenge_invalid",
            "2FA challenge is expired or already used",
        );
    }

    let user_id = challenge.user_id;
    match verify_two_factor_for_user(&state, user_id, &request.code).await {
        Ok(true) => {
            let mut conn = state.db_pool.get().expect("Failed to get DB connection");
            tokio::task::spawn_blocking(move || {
                diesel::update(
                    login_two_factor_challenges::table
                        .filter(login_two_factor_challenges::id.eq(challenge.id)),
                )
                .set(login_two_factor_challenges::consumed_at.eq(Some(now)))
                .execute(&mut conn)
            })
            .await
            .expect("Failed to spawn blocking task")
            .expect("Failed to consume 2FA challenge");

            if let Err(msg) = issue_session_cookie(&state, &cookies, user_id).await {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "session_error",
                    msg.as_str(),
                );
            }

            (
                StatusCode::OK,
                Json(OAuthResponse {
                    status: "success",
                    message: "2FA verification succeeded".to_string(),
                    challenge_id: None,
                    login_two_factor_challenge_id: None,
                }),
            )
        }
        Ok(false) => error_response(
            StatusCode::UNAUTHORIZED,
            "two_factor_invalid",
            "Invalid 2FA code",
        ),
        Err(msg) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "two_factor_error",
            msg.as_str(),
        ),
    }
}

async fn set_admin_two_factor_requirement(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<AdminTwoFactorRequirementRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    if let Err(resp) = verify_admin_request(&state, &cookies).await {
        return resp;
    }

    let now = Utc::now().naive_utc();
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let required = request.required_for_all_users;

    let result = tokio::task::spawn_blocking(move || {
        diesel::update(auth_security_settings::table.filter(auth_security_settings::id.eq(1)))
            .set((
                auth_security_settings::two_factor_required_for_all_users.eq(required),
                auth_security_settings::updated_at.eq(now),
            ))
            .execute(&mut conn)
    })
    .await
    .expect("Failed to spawn blocking task");

    if result.is_err() {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "settings_error",
            "Failed to update global 2FA requirement",
        );
    }

    (
        StatusCode::OK,
        Json(OAuthResponse {
            status: "success",
            message: "Global 2FA requirement updated".to_string(),
            challenge_id: None,
            login_two_factor_challenge_id: None,
        }),
    )
}

async fn set_admin_user_two_factor_required(
    cookies: Cookies,
    Path(user_id): Path<uuid::Uuid>,
    State(state): State<AppState>,
    Json(request): Json<AdminUserTwoFactorRequiredRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    if let Err(resp) = verify_admin_request(&state, &cookies).await {
        return resp;
    }

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let required = request.required;
    let result = tokio::task::spawn_blocking(move || {
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::two_factor_admin_required.eq(required))
            .execute(&mut conn)
    })
    .await
    .expect("Failed to spawn blocking task");

    match result {
        Ok(0) => error_response(StatusCode::NOT_FOUND, "not_found", "User not found"),
        Ok(_) => (
            StatusCode::OK,
            Json(OAuthResponse {
                status: "success",
                message: "User 2FA requirement updated".to_string(),
                challenge_id: None,
                login_two_factor_challenge_id: None,
            }),
        ),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "settings_error",
            "Failed to update user 2FA requirement",
        ),
    }
}
async fn logout(
    cookies: Cookies,
    State(state): State<AppState>,
) -> (StatusCode, Json<AuthSessionResponse>) {
    if let Some(session_cookie) = cookies.private(&state.key).get("session")
        && let Ok(session_id) = uuid::Uuid::parse_str(session_cookie.value())
    {
        let now = Utc::now().naive_utc();
        if let Ok(mut conn) = state.db_pool.get() {
            let _ = tokio::task::spawn_blocking(move || {
                diesel::update(user_sessions::table.filter(user_sessions::id.eq(session_id)))
                    .set(user_sessions::revoked_at.eq(Some(now)))
                    .execute(&mut conn)
            })
            .await;
        }
    }

    cookies
        .private(&state.key)
        .remove(Cookie::new("session", ""));
    cookies.remove(Cookie::new("csrf_token", ""));

    (
        StatusCode::OK,
        Json(AuthSessionResponse {
            status: "logged_out",
            message: "Session cleared".to_string(),
            session: None,
            login_two_factor_challenge_id: None,
            requires_email_verification: false,
        }),
    )
}

async fn oauth_start(
    Path(provider_key): Path<String>,
    Query(query): Query<OAuthStartQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<OAuthResponse>)> {
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let provider_key_clone = provider_key.clone();
    let now = Utc::now().naive_utc();
    let state_token = random_urlsafe(32);
    let code_verifier = random_urlsafe(48);

    let provider = tokio::task::spawn_blocking(move || {
        oauth_providers::table
            .filter(oauth_providers::provider_key.eq(provider_key_clone))
            .filter(oauth_providers::enabled.eq(true))
            .select(OAuthProvider::as_select())
            .first::<OAuthProvider>(&mut conn)
            .optional()
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query DB")
    .ok_or_else(|| {
        error_response(
            StatusCode::NOT_FOUND,
            "provider_not_found",
            "OAuth provider is not configured or disabled",
        )
    })?;

    if !provider.configured {
        return Err(error_response(
            StatusCode::CONFLICT,
            "provider_not_configured",
            "Provider exists but is not configured with client credentials",
        ));
    }

    let Some(provider_client_id) = provider.oauth_client_id.clone() else {
        return Err(error_response(
            StatusCode::CONFLICT,
            "provider_not_configured",
            "Provider client id is not set",
        ));
    };

    let session = NewOAuthAuthorizationSession {
        id: uuid::Uuid::now_v7(),
        provider_id: provider.id,
        oauth_provider_key: provider_key.clone(),
        oauth_client_id: provider_client_id.clone(),
        state: state_token.clone(),
        code_verifier: code_verifier.clone(),
        redirect_uri: query.redirect_uri.clone(),
        return_to: query.return_to,
        expires_at: now + chrono::Duration::minutes(10),
        consumed_at: None,
        created_at: now,
    };

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    tokio::task::spawn_blocking(move || {
        diesel::insert_into(oauth_authorization_sessions::table)
            .values(&session)
            .execute(&mut conn)
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to persist oauth authorization session");

    let code_challenge = code_challenge_from_verifier(&code_verifier);
    let mut url = Url::parse(&provider.authorization_url).map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "provider_misconfigured",
            "Provider authorization URL is invalid",
        )
    })?;

    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &provider_client_id)
        .append_pair("redirect_uri", &query.redirect_uri)
        .append_pair(
            "scope",
            &provider
                .scopes
                .iter()
                .filter_map(|s| s.as_ref())
                .cloned()
                .collect::<Vec<String>>()
                .join(" "),
        )
        .append_pair("state", &state_token)
        .append_pair("code_challenge", &code_challenge)
        .append_pair("code_challenge_method", "S256");

    Ok(Redirect::temporary(url.as_str()))
}

async fn oauth_callback(
    Path(provider_key): Path<String>,
    Query(query): Query<OAuthCallbackQuery>,
    cookies: Cookies,
    State(state): State<AppState>,
) -> (StatusCode, Json<OAuthResponse>) {
    if let Some(err) = query.error {
        return (
            StatusCode::BAD_REQUEST,
            Json(OAuthResponse {
                status: "oauth_error",
                message: format!(
                    "Provider returned error '{}': {}",
                    err,
                    query
                        .error_description
                        .unwrap_or_else(|| "unknown".to_string())
                ),
                challenge_id: None,
                login_two_factor_challenge_id: None,
            }),
        );
    }

    let Some(code) = query.code else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Missing 'code' query parameter",
        );
    };
    let Some(state_token) = query.state else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Missing 'state' query parameter",
        );
    };

    handle_oauth_code_flow(state, cookies, provider_key, state_token, code).await
}

async fn oauth_token_exchange(
    Path(provider_key): Path<String>,
    State(state): State<AppState>,
    cookies: Cookies,
    Json(payload): Json<OAuthTokenExchangeRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    handle_oauth_code_flow(state, cookies, provider_key, payload.state, payload.code).await
}

async fn oauth_resolve(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<OAuthResolveRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    resolve_oauth_login(state, cookies, request).await
}

async fn handle_oauth_code_flow(
    state: AppState,
    cookies: Cookies,
    provider_key: String,
    state_token: String,
    code: String,
) -> (StatusCode, Json<OAuthResponse>) {
    let auth_ctx =
        match load_and_consume_authorization_session(&state, &provider_key, &state_token).await {
            Ok(ctx) => ctx,
            Err(resp) => return resp,
        };

    let token_response = match exchange_authorization_code(&auth_ctx, &code).await {
        Ok(tokens) => tokens,
        Err(msg) => {
            return error_response(
                StatusCode::BAD_GATEWAY,
                "token_exchange_failed",
                msg.as_str(),
            );
        }
    };

    let userinfo = if let Some(userinfo_url) = auth_ctx.provider.userinfo_url.clone() {
        match fetch_userinfo(userinfo_url, token_response.access_token.clone()).await {
            Ok(v) => Some(v),
            Err(msg) => {
                return error_response(StatusCode::BAD_GATEWAY, "userinfo_failed", msg.as_str());
            }
        }
    } else {
        None
    };

    let provider_user_id = userinfo
        .as_ref()
        .and_then(extract_provider_user_id)
        .or_else(|| extract_provider_user_id_from_token(&token_response));

    let Some(provider_user_id) = provider_user_id else {
        return error_response(
            StatusCode::BAD_GATEWAY,
            "identity_missing",
            "Could not extract provider user id from provider response",
        );
    };

    let provider_email = userinfo.as_ref().and_then(extract_provider_email);
    let expires_at = token_response
        .expires_in
        .map(|seconds| Utc::now() + chrono::Duration::seconds(seconds));

    let resolve_request = OAuthResolveRequest {
        provider_key,
        provider_user_id,
        provider_email,
        access_token: Some(token_response.access_token),
        refresh_token: token_response.refresh_token,
        token_expires_at: expires_at,
    };

    resolve_oauth_login(state, cookies, resolve_request).await
}

async fn resolve_oauth_login(
    state: AppState,
    cookies: Cookies,
    request: OAuthResolveRequest,
) -> (StatusCode, Json<OAuthResponse>) {
    let encryption_key = match encryption_key_from_config_secret(&state.config.secret) {
        Ok(key) => key,
        Err(msg) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "encryption_key_invalid",
                msg.as_str(),
            );
        }
    };

    let access_token_encrypted = match request
        .access_token
        .as_deref()
        .map(|v| encrypt_secret(v, &encryption_key))
        .transpose()
    {
        Ok(v) => v,
        Err(msg) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "encryption_failed",
                msg.as_str(),
            );
        }
    };

    let refresh_token_encrypted = match request
        .refresh_token
        .as_deref()
        .map(|v| encrypt_secret(v, &encryption_key))
        .transpose()
    {
        Ok(v) => v,
        Err(msg) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "encryption_failed",
                msg.as_str(),
            );
        }
    };

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let now = Utc::now().naive_utc();
    let provider_key = request.provider_key;
    let provider_key_for_audit = provider_key.clone();
    let provider_user_id = request.provider_user_id;
    let provider_email = request
        .provider_email
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty());
    let token_expires_at = request.token_expires_at.map(|v| v.naive_utc());

    let outcome =
        tokio::task::spawn_blocking(move || -> Result<ResolveOutcome, diesel::result::Error> {
            let provider = oauth_providers::table
                .filter(oauth_providers::provider_key.eq(&provider_key))
                .filter(oauth_providers::enabled.eq(true))
                .select(OAuthProvider::as_select())
                .first::<OAuthProvider>(&mut conn)
                .optional()?;

            let Some(provider) = provider else {
                return Ok(ResolveOutcome::ProviderNotFound);
            };

            let existing_link = user_oauth_accounts::table
                .filter(user_oauth_accounts::provider_id.eq(provider.id))
                .filter(user_oauth_accounts::provider_user_id.eq(&provider_user_id))
                .select(UserOAuthAccount::as_select())
                .first::<UserOAuthAccount>(&mut conn)
                .optional()?;

            if let Some(existing_link) = existing_link {
                diesel::update(
                    user_oauth_accounts::table.filter(user_oauth_accounts::id.eq(existing_link.id)),
                )
                .set((
                    user_oauth_accounts::provider_email.eq(provider_email.clone()),
                    user_oauth_accounts::access_token_encrypted.eq(access_token_encrypted.clone()),
                    user_oauth_accounts::refresh_token_encrypted
                        .eq(refresh_token_encrypted.clone()),
                    user_oauth_accounts::token_expires_at.eq(token_expires_at),
                    user_oauth_accounts::updated_at.eq(now),
                ))
                .execute(&mut conn)?;

                return Ok(ResolveOutcome::LinkedUser(existing_link.user_id));
            }

            let Some(provider_email) = provider_email else {
                return Ok(ResolveOutcome::NoMatchingUser);
            };

            let existing_user_id = users::table
                .filter(users::email.eq(&provider_email))
                .select(users::id)
                .first::<uuid::Uuid>(&mut conn)
                .optional()?;

            let Some(existing_user_id) = existing_user_id else {
                // ADR-011: unlike an existing local account (which still
                // requires password confirmation below before linking), a
                // first-time OAuth identity with no local account at all is
                // auto-provisioned. The provider already vouched for this
                // email, so there is no password to protect and no ambiguity
                // about which account to link.
                let username = unique_username_from_email_sync(&mut conn, &provider_email)?;
                let random_password_hash = hash_password(&random_urlsafe(48))
                    .expect("Failed to hash random password for auto-provisioned OAuth user");

                let new_user_id = uuid::Uuid::now_v7();
                diesel::insert_into(users::table)
                    .values((
                        users::id.eq(new_user_id),
                        users::username.eq(&username),
                        users::email.eq(&provider_email),
                        users::is_admin.eq(false),
                        users::password_hash.eq(random_password_hash),
                        users::created_at.eq(now),
                        users::updated_at.eq(now),
                        users::two_factor_enabled.eq(false),
                        users::two_factor_secret_encrypted.eq::<Option<String>>(None),
                        users::two_factor_confirmed_at.eq::<Option<chrono::NaiveDateTime>>(None),
                        users::two_factor_admin_required.eq(false),
                    ))
                    .execute(&mut conn)?;

                let oauth_account = NewUserOAuthAccount {
                    id: uuid::Uuid::now_v7(),
                    user_id: new_user_id,
                    provider_id: provider.id,
                    provider_user_id,
                    provider_email: Some(provider_email),
                    access_token_encrypted,
                    refresh_token_encrypted,
                    token_expires_at,
                    linked_at: now,
                    created_at: now,
                    updated_at: now,
                };

                diesel::insert_into(user_oauth_accounts::table)
                    .values(&oauth_account)
                    .execute(&mut conn)?;

                return Ok(ResolveOutcome::LinkedUser(new_user_id));
            };

            let challenge_id = uuid::Uuid::now_v7();
            let challenge = NewOAuthLinkChallenge {
                id: challenge_id,
                user_id: existing_user_id,
                provider_id: provider.id,
                provider_user_id,
                provider_email: Some(provider_email),
                challenge_code: uuid::Uuid::now_v7().to_string(),
                expires_at: now + chrono::Duration::minutes(10),
                consumed_at: None,
                pending_access_token_encrypted: access_token_encrypted,
                pending_refresh_token_encrypted: refresh_token_encrypted,
                pending_token_expires_at: token_expires_at,
                created_at: now,
            };

            diesel::insert_into(oauth_link_challenges::table)
                .values(&challenge)
                .execute(&mut conn)?;

            Ok(ResolveOutcome::PasswordRequired(challenge_id))
        })
        .await
        .expect("Failed to spawn blocking task")
        .expect("Failed to resolve oauth login");

    match outcome {
        ResolveOutcome::ProviderNotFound => error_response(
            StatusCode::NOT_FOUND,
            "provider_not_found",
            "OAuth provider is not configured or disabled",
        ),
        ResolveOutcome::LinkedUser(user_id) => {
            let two_factor_required = match is_two_factor_required_for_user(&state, user_id).await {
                Ok(v) => v,
                Err(msg) => {
                    return error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "two_factor_error",
                        msg.as_str(),
                    );
                }
            };

            if two_factor_required {
                let challenge_id = match create_login_two_factor_challenge(&state, user_id).await {
                    Ok(v) => v,
                    Err(msg) => {
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "two_factor_error",
                            msg.as_str(),
                        );
                    }
                };

                return (
                    StatusCode::UNAUTHORIZED,
                    Json(OAuthResponse {
                        status: "two_factor_required",
                        message: "2FA code required to complete sign-in".to_string(),
                        challenge_id: None,
                        login_two_factor_challenge_id: Some(challenge_id),
                    }),
                );
            }

            if let Err(msg) = issue_session_cookie(&state, &cookies, user_id).await {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "session_error",
                    msg.as_str(),
                );
            }

            (
                StatusCode::OK,
                Json(OAuthResponse {
                    status: "success",
                    message: "OAuth account is already linked and signed in".to_string(),
                    challenge_id: None,
                    login_two_factor_challenge_id: None,
                }),
            )
        }
        ResolveOutcome::PasswordRequired(challenge_id) => {
            let _ = record_auth_audit_event(
                &state,
                None,
                "oauth_link_challenge_issued",
                None,
                Some(serde_json::json!({
                    "challenge_id": challenge_id,
                    "provider_key": provider_key_for_audit,
                })),
            )
            .await;

            (
                StatusCode::CONFLICT,
                Json(OAuthResponse {
                    status: "password_required",
                    message:
                        "Existing account detected; confirm password to link this OAuth account"
                            .to_string(),
                    challenge_id: Some(challenge_id),
                    login_two_factor_challenge_id: None,
                }),
            )
        }
        ResolveOutcome::NoMatchingUser => error_response(
            StatusCode::NOT_FOUND,
            "no_matching_user",
            "The OAuth provider did not return an email address, so this identity cannot be linked or auto-provisioned",
        ),
    }
}

async fn oauth_link_confirm(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<OAuthLinkConfirmRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    let challenge_id = request.challenge_id;
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let now = Utc::now().naive_utc();

    let outcome = tokio::task::spawn_blocking(
        move || -> Result<LinkConfirmOutcome, diesel::result::Error> {
            let challenge = oauth_link_challenges::table
                .filter(oauth_link_challenges::id.eq(request.challenge_id))
                .select(OAuthLinkChallenge::as_select())
                .first::<OAuthLinkChallenge>(&mut conn)
                .optional()?;

            let Some(challenge) = challenge else {
                return Ok(LinkConfirmOutcome::ChallengeInvalid);
            };

            if challenge.consumed_at.is_some() {
                return Ok(LinkConfirmOutcome::ChallengeInvalid);
            }

            if challenge.expires_at <= now {
                return Ok(LinkConfirmOutcome::ChallengeExpired);
            }

            let password_hash = users::table
                .filter(users::id.eq(challenge.user_id))
                .select(users::password_hash)
                .first::<String>(&mut conn)
                .optional()?;

            let Some(password_hash) = password_hash else {
                return Ok(LinkConfirmOutcome::ChallengeInvalid);
            };

            let parsed_hash = PasswordHash::new(&password_hash).expect("Invalid hash in db");
            if Argon2::default()
                .verify_password(request.password.as_bytes(), &parsed_hash)
                .is_err()
            {
                return Ok(LinkConfirmOutcome::PasswordMismatch);
            }

            let account_for_subject = user_oauth_accounts::table
                .filter(user_oauth_accounts::provider_id.eq(challenge.provider_id))
                .filter(user_oauth_accounts::provider_user_id.eq(&challenge.provider_user_id))
                .select(UserOAuthAccount::as_select())
                .first::<UserOAuthAccount>(&mut conn)
                .optional()?;

            if let Some(account_for_subject) = account_for_subject.as_ref()
                && account_for_subject.user_id != challenge.user_id
            {
                return Ok(LinkConfirmOutcome::LinkConflict);
            }

            let account_for_user_provider = user_oauth_accounts::table
                .filter(user_oauth_accounts::user_id.eq(challenge.user_id))
                .filter(user_oauth_accounts::provider_id.eq(challenge.provider_id))
                .select(UserOAuthAccount::as_select())
                .first::<UserOAuthAccount>(&mut conn)
                .optional()?;

            if account_for_user_provider.is_none() && account_for_subject.is_none() {
                let account = NewUserOAuthAccount {
                    id: uuid::Uuid::now_v7(),
                    user_id: challenge.user_id,
                    provider_id: challenge.provider_id,
                    provider_user_id: challenge.provider_user_id,
                    provider_email: challenge.provider_email,
                    access_token_encrypted: challenge.pending_access_token_encrypted,
                    refresh_token_encrypted: challenge.pending_refresh_token_encrypted,
                    token_expires_at: challenge.pending_token_expires_at,
                    linked_at: now,
                    created_at: now,
                    updated_at: now,
                };

                diesel::insert_into(user_oauth_accounts::table)
                    .values(&account)
                    .execute(&mut conn)?;
            }

            diesel::update(
                oauth_link_challenges::table.filter(oauth_link_challenges::id.eq(challenge.id)),
            )
            .set(oauth_link_challenges::consumed_at.eq(Some(now)))
            .execute(&mut conn)?;

            Ok(LinkConfirmOutcome::Linked(challenge.user_id))
        },
    )
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to confirm oauth link");

    match outcome {
        LinkConfirmOutcome::ChallengeInvalid => error_response(
            StatusCode::BAD_REQUEST,
            "invalid_challenge",
            "Challenge is invalid or already consumed",
        ),
        LinkConfirmOutcome::ChallengeExpired => error_response(
            StatusCode::BAD_REQUEST,
            "challenge_expired",
            "Challenge has expired; restart OAuth sign-in",
        ),
        LinkConfirmOutcome::PasswordMismatch => error_response(
            StatusCode::UNAUTHORIZED,
            "password_mismatch",
            "Password verification failed",
        ),
        LinkConfirmOutcome::LinkConflict => error_response(
            StatusCode::CONFLICT,
            "link_conflict",
            "That OAuth identity is already linked to a different account",
        ),
        LinkConfirmOutcome::Linked(user_id) => {
            if let Err(msg) = issue_session_cookie(&state, &cookies, user_id).await {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "session_error",
                    msg.as_str(),
                );
            }

            let _ = record_auth_audit_event(
                &state,
                Some(user_id),
                "oauth_link_confirmed",
                None,
                Some(serde_json::json!({
                    "challenge_id": challenge_id,
                })),
            )
            .await;

            (
                StatusCode::OK,
                Json(OAuthResponse {
                    status: "success",
                    message: "OAuth account linked successfully".to_string(),
                    challenge_id: None,
                    login_two_factor_challenge_id: None,
                }),
            )
        }
    }
}

async fn load_and_consume_authorization_session(
    state: &AppState,
    provider_key: &str,
    state_token: &str,
) -> Result<OAuthAuthorizationContext, (StatusCode, Json<OAuthResponse>)> {
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let provider_key = provider_key.to_string();
    let state_token = state_token.to_string();
    let now = Utc::now().naive_utc();

    let result = tokio::task::spawn_blocking(
        move || -> Result<Option<OAuthAuthorizationContext>, diesel::result::Error> {
            let session = oauth_authorization_sessions::table
                .filter(oauth_authorization_sessions::oauth_provider_key.eq(&provider_key))
                .filter(oauth_authorization_sessions::state.eq(&state_token))
                .select(OAuthAuthorizationSession::as_select())
                .first::<OAuthAuthorizationSession>(&mut conn)
                .optional()?;

            let Some(session) = session else {
                return Ok(None);
            };

            if session.consumed_at.is_some() || session.expires_at <= now {
                return Ok(None);
            }

            let provider = oauth_providers::table
                .filter(oauth_providers::id.eq(session.provider_id))
                .filter(oauth_providers::enabled.eq(true))
                .filter(oauth_providers::configured.eq(true))
                .select(OAuthProvider::as_select())
                .first::<OAuthProvider>(&mut conn)
                .optional()?;

            let Some(provider) = provider else {
                return Ok(None);
            };

            diesel::update(
                oauth_authorization_sessions::table
                    .filter(oauth_authorization_sessions::id.eq(session.id)),
            )
            .set(oauth_authorization_sessions::consumed_at.eq(Some(now)))
            .execute(&mut conn)?;

            Ok(Some(OAuthAuthorizationContext { provider, session }))
        },
    )
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query oauth authorization session");

    result.ok_or_else(|| {
        error_response(
            StatusCode::BAD_REQUEST,
            "invalid_oauth_state",
            "OAuth state is invalid, expired, or already consumed",
        )
    })
}

async fn exchange_authorization_code(
    auth_ctx: &OAuthAuthorizationContext,
    code: &str,
) -> Result<OAuthTokenResponse, String> {
    exchange_authorization_code_with_provider(
        &auth_ctx.provider,
        &auth_ctx.session.redirect_uri,
        &auth_ctx.session.code_verifier,
        code,
    )
    .await
}

async fn exchange_authorization_code_with_provider(
    provider: &OAuthProvider,
    redirect_uri: &str,
    code_verifier: &str,
    code: &str,
) -> Result<OAuthTokenResponse, String> {
    let Some(client_id) = provider.oauth_client_id.as_deref() else {
        return Err("Provider client id is not configured".to_string());
    };
    let Some(client_secret) = provider.oauth_client_secret.as_deref() else {
        return Err("Provider client secret is not configured".to_string());
    };

    let client = reqwest::Client::new();
    let response = client
        .post(&provider.token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .map_err(|e| format!("Token request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<no body>".to_string());
        return Err(format!(
            "Token exchange failed with status {status}: {body}"
        ));
    }

    response
        .json::<OAuthTokenResponse>()
        .await
        .map_err(|e| format!("Invalid token response format: {e}"))
}

pub async fn ensure_admin_bootstrap_code(state: &AppState) -> Result<(), String> {
    let now = Utc::now().naive_utc();
    let bootstrap_code = random_setup_code();
    let bootstrap_code_hash = hash_password(&bootstrap_code)?;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    let generated_code = tokio::task::spawn_blocking(move || -> Result<Option<String>, String> {
        let admin_exists = users::table
            .filter(users::is_admin.eq(true))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to query admin users".to_string())?;

        let existing = admin_bootstrap_setup::table
            .filter(admin_bootstrap_setup::id.eq(1))
            .select(AdminBootstrapSetup::as_select())
            .first::<AdminBootstrapSetup>(&mut conn)
            .optional()
            .map_err(|_| "Failed to load bootstrap setup state".to_string())?;

        if admin_exists.is_some() {
            if existing.is_some() {
                mark_admin_setup_complete_sync(&mut conn, now)?;
            } else {
                let new_row = NewAdminBootstrapSetup {
                    id: 1,
                    setup_completed_at: Some(now),
                    admin_code_hash: None,
                    admin_code_generated_at: None,
                    created_at: now,
                    updated_at: now,
                };
                diesel::insert_into(admin_bootstrap_setup::table)
                    .values(&new_row)
                    .execute(&mut conn)
                    .map_err(|_| "Failed to persist bootstrap setup state".to_string())?;
            }

            return Ok(None);
        }

        if existing.is_some() {
            diesel::update(admin_bootstrap_setup::table.filter(admin_bootstrap_setup::id.eq(1)))
                .set((
                    admin_bootstrap_setup::setup_completed_at
                        .eq::<Option<chrono::NaiveDateTime>>(None),
                    admin_bootstrap_setup::admin_code_hash.eq(Some(bootstrap_code_hash)),
                    admin_bootstrap_setup::admin_code_generated_at.eq(Some(now)),
                    admin_bootstrap_setup::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .map_err(|_| "Failed to update bootstrap setup state".to_string())?;
        } else {
            let new_row = NewAdminBootstrapSetup {
                id: 1,
                setup_completed_at: None,
                admin_code_hash: Some(bootstrap_code_hash),
                admin_code_generated_at: Some(now),
                created_at: now,
                updated_at: now,
            };
            diesel::insert_into(admin_bootstrap_setup::table)
                .values(&new_row)
                .execute(&mut conn)
                .map_err(|_| "Failed to persist bootstrap setup state".to_string())?;
        }

        Ok(Some(bootstrap_code))
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())??;

    if let Some(bootstrap_code) = generated_code {
        tracing::warn!(
            "Initial admin setup is incomplete. To create an admin account, visit: http://127.0.0.1:5173/setup/{}",
            bootstrap_code
        );
    }

    Ok(())
}

async fn ensure_admin_setup_code_valid(
    state: &AppState,
    admin_code: &str,
) -> Result<(), (StatusCode, Json<OAuthResponse>)> {
    let code = admin_code.trim().to_string();
    if code.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Admin setup code is required",
        ));
    }

    let mut conn = state.db_pool.get().map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "setup_error",
            "Failed to get DB connection",
        )
    })?;

    let result = tokio::task::spawn_blocking(move || -> Result<Result<(), String>, String> {
        let admin_exists = users::table
            .filter(users::is_admin.eq(true))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to query admin users".to_string())?;
        if admin_exists.is_some() {
            return Ok(Err("Setup has already been completed".to_string()));
        }

        let setup = admin_bootstrap_setup::table
            .filter(admin_bootstrap_setup::id.eq(1))
            .select(AdminBootstrapSetup::as_select())
            .first::<AdminBootstrapSetup>(&mut conn)
            .optional()
            .map_err(|_| "Failed to load bootstrap setup state".to_string())?;

        let Some(setup) = setup else {
            return Ok(Err("Setup state is not initialized yet".to_string()));
        };

        if setup.setup_completed_at.is_some() {
            return Ok(Err("Setup has already been completed".to_string()));
        }

        let Some(admin_code_hash) = setup.admin_code_hash else {
            return Ok(Err("Bootstrap admin code is not active".to_string()));
        };

        let parsed_hash = PasswordHash::new(&admin_code_hash)
            .map_err(|_| "Stored bootstrap admin code hash is invalid".to_string())?;
        if Argon2::default()
            .verify_password(code.as_bytes(), &parsed_hash)
            .is_err()
        {
            return Ok(Err("Invalid bootstrap admin code".to_string()));
        }

        Ok(Ok(()))
    })
    .await
    .map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "setup_error",
            "Failed to spawn blocking task",
        )
    })
    .and_then(|r| {
        r.map_err(|msg| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "setup_error",
                msg.as_str(),
            )
        })
    })?;

    match result {
        Ok(()) => Ok(()),
        Err(msg) if msg == "Setup has already been completed" => Err(error_response(
            StatusCode::CONFLICT,
            "setup_complete",
            msg.as_str(),
        )),
        Err(msg) if msg == "Invalid bootstrap admin code" => Err(error_response(
            StatusCode::UNAUTHORIZED,
            "invalid_admin_code",
            msg.as_str(),
        )),
        Err(msg) => Err(error_response(
            StatusCode::BAD_REQUEST,
            "setup_unavailable",
            msg.as_str(),
        )),
    }
}

async fn load_and_consume_admin_bootstrap_oauth_session(
    state: &AppState,
    provider_key: &str,
    state_token: &str,
) -> Result<AdminBootstrapOAuthContext, (StatusCode, Json<OAuthResponse>)> {
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let provider_key = provider_key.to_string();
    let state_token = state_token.to_string();
    let now = Utc::now().naive_utc();

    let result = tokio::task::spawn_blocking(
        move || -> Result<Option<AdminBootstrapOAuthContext>, diesel::result::Error> {
            let session = admin_bootstrap_oauth_sessions::table
                .filter(admin_bootstrap_oauth_sessions::oauth_provider_key.eq(&provider_key))
                .filter(admin_bootstrap_oauth_sessions::state.eq(&state_token))
                .select(AdminBootstrapOAuthSession::as_select())
                .first::<AdminBootstrapOAuthSession>(&mut conn)
                .optional()?;

            let Some(session) = session else {
                return Ok(None);
            };

            if session.consumed_at.is_some() || session.expires_at <= now {
                return Ok(None);
            }

            let provider = oauth_providers::table
                .filter(oauth_providers::id.eq(session.provider_id))
                .filter(oauth_providers::enabled.eq(true))
                .filter(oauth_providers::configured.eq(true))
                .select(OAuthProvider::as_select())
                .first::<OAuthProvider>(&mut conn)
                .optional()?;

            let Some(provider) = provider else {
                return Ok(None);
            };

            diesel::update(
                admin_bootstrap_oauth_sessions::table
                    .filter(admin_bootstrap_oauth_sessions::id.eq(session.id)),
            )
            .set(admin_bootstrap_oauth_sessions::consumed_at.eq(Some(now)))
            .execute(&mut conn)?;

            Ok(Some(AdminBootstrapOAuthContext { provider, session }))
        },
    )
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query bootstrap oauth authorization session");

    result.ok_or_else(|| {
        error_response(
            StatusCode::BAD_REQUEST,
            "invalid_oauth_state",
            "Bootstrap OAuth state is invalid, expired, or already consumed",
        )
    })
}

async fn fetch_userinfo(url: String, access_token: String) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("Userinfo request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<no body>".to_string());
        return Err(format!("Userinfo failed with status {status}: {body}"));
    }

    response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("Invalid userinfo response format: {e}"))
}

fn extract_provider_user_id(userinfo: &serde_json::Value) -> Option<String> {
    ["sub", "id", "user_id"]
        .iter()
        .find_map(|key| userinfo.get(*key))
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

fn extract_provider_user_id_from_token(token: &OAuthTokenResponse) -> Option<String> {
    let _ = token;
    None
}

fn extract_provider_email(userinfo: &serde_json::Value) -> Option<String> {
    userinfo
        .get("email")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
}

async fn issue_session_cookie(
    state: &AppState,
    cookies: &Cookies,
    user_id: uuid::Uuid,
) -> Result<crate::models::UserSession, String> {
    let now = Utc::now().naive_utc();
    let session_id = uuid::Uuid::now_v7();
    let session_ttl_days = 7;
    let expires_at = now + chrono::Duration::days(session_ttl_days);
    let new_session = NewUserSession {
        id: session_id,
        user_id,
        expires_at,
        revoked_at: None,
        created_at: now,
    };

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection")?;
    tokio::task::spawn_blocking(move || {
        // Revoke existing active sessions on new login to reduce session replay risk.
        diesel::update(
            user_sessions::table
                .filter(user_sessions::user_id.eq(user_id))
                .filter(user_sessions::revoked_at.is_null()),
        )
        .set(user_sessions::revoked_at.eq(Some(now)))
        .execute(&mut conn)?;

        diesel::insert_into(user_sessions::table)
            .values(&new_session)
            .execute(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())
    .and_then(|r| r.map_err(|_| "Failed to persist user session".to_string()))?;

    let mut cookie = Cookie::new("session", session_id.to_string());
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Strict);
    cookie.set_secure(state.config.secure_cookies);
    cookies.private(&state.key).add(cookie);

    let mut csrf_cookie = Cookie::new("csrf_token", uuid::Uuid::now_v7().to_string());
    csrf_cookie.set_path("/");
    csrf_cookie.set_http_only(false);
    csrf_cookie.set_same_site(SameSite::Strict);
    csrf_cookie.set_secure(state.config.secure_cookies);
    cookies.add(csrf_cookie);
    Ok(crate::models::UserSession {
        id: session_id,
        user_id,
        expires_at,
        revoked_at: None,
        created_at: now,
    })
}

async fn load_global_two_factor_requirement(state: &AppState) -> Result<bool, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection")?;
    let result = tokio::task::spawn_blocking(move || {
        auth_security_settings::table
            .filter(auth_security_settings::id.eq(1))
            .select(AuthSecuritySetting::as_select())
            .first::<AuthSecuritySetting>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())
    .and_then(|r| r.map_err(|_| "Failed to query auth security settings".to_string()))?;

    Ok(result
        .map(|s| s.two_factor_required_for_all_users)
        .unwrap_or(false))
}

async fn is_two_factor_required_for_user(
    state: &AppState,
    user_id: uuid::Uuid,
) -> Result<bool, String> {
    let global_required = load_global_two_factor_requirement(state).await?;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection")?;
    let local = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::id.eq(user_id))
            .select((users::two_factor_enabled, users::two_factor_admin_required))
            .first::<(bool, bool)>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())
    .and_then(|r| r.map_err(|_| "Failed to query user 2FA state".to_string()))?;

    let Some((enabled, admin_required)) = local else {
        return Ok(false);
    };

    Ok(global_required || enabled || admin_required)
}

async fn create_login_two_factor_challenge(
    state: &AppState,
    user_id: uuid::Uuid,
) -> Result<uuid::Uuid, String> {
    let now = Utc::now().naive_utc();
    let challenge_id = uuid::Uuid::now_v7();
    let challenge = NewLoginTwoFactorChallenge {
        id: challenge_id,
        user_id,
        expires_at: now + chrono::Duration::minutes(10),
        consumed_at: None,
        created_at: now,
    };

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection")?;
    tokio::task::spawn_blocking(move || {
        diesel::insert_into(login_two_factor_challenges::table)
            .values(&challenge)
            .execute(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())
    .and_then(|r| r.map_err(|_| "Failed to create 2FA challenge".to_string()))?;

    Ok(challenge_id)
}

async fn verify_two_factor_for_user(
    state: &AppState,
    user_id: uuid::Uuid,
    code: &str,
) -> Result<bool, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection")?;
    let user = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::id.eq(user_id))
            .select((users::username, users::two_factor_secret_encrypted))
            .first::<(String, Option<String>)>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())
    .and_then(|r| r.map_err(|_| "Failed to query user for 2FA".to_string()))?;

    let Some((username, secret_encrypted)) = user else {
        return Ok(false);
    };
    let Some(secret_encrypted) = secret_encrypted else {
        return Ok(false);
    };

    let encryption_key = encryption_key_from_config_secret(&state.config.secret)?;
    let secret = decrypt_secret(&secret_encrypted, &encryption_key)?;
    verify_totp_code(&username, &secret, code)
}

fn verify_totp_code(username: &str, secret_base32: &str, code: &str) -> Result<bool, String> {
    let secret = BASE32_NOPAD
        .decode(secret_base32.as_bytes())
        .map_err(|_| "Stored 2FA secret is not valid base32".to_string())?;
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret,
        Some("ThunderForge".to_string()),
        username.to_string(),
    )
    .map_err(|e| format!("Failed to build TOTP verifier: {e}"))?;
    totp.check_current(code)
        .map_err(|e| format!("Failed to validate TOTP code: {e}"))
}

fn decrypt_secret(ciphertext: &str, key: &[u8; 32]) -> Result<String, String> {
    let mut parts = ciphertext.split('.');
    let version = parts
        .next()
        .ok_or_else(|| "Invalid encrypted secret format".to_string())?;
    if version != "v1" {
        return Err("Unsupported encrypted secret version".to_string());
    }
    let nonce_b64 = parts
        .next()
        .ok_or_else(|| "Invalid encrypted secret format".to_string())?;
    let cipher_b64 = parts
        .next()
        .ok_or_else(|| "Invalid encrypted secret format".to_string())?;
    if parts.next().is_some() {
        return Err("Invalid encrypted secret format".to_string());
    }

    let nonce_vec = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(nonce_b64)
        .map_err(|_| "Invalid encrypted secret nonce".to_string())?;
    if nonce_vec.len() != 12 {
        return Err("Invalid encrypted secret nonce length".to_string());
    }
    let cipher_vec = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cipher_b64)
        .map_err(|_| "Invalid encrypted secret payload".to_string())?;

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("Cipher init failed: {e}"))?;
    let nonce = Nonce::from_slice(&nonce_vec);
    let plaintext = cipher
        .decrypt(nonce, cipher_vec.as_ref())
        .map_err(|e| format!("Decryption failed: {e}"))?;

    String::from_utf8(plaintext).map_err(|_| "Decrypted secret is not valid UTF-8".to_string())
}

async fn verify_admin_request(
    state: &AppState,
    cookies: &Cookies,
) -> Result<(), (StatusCode, Json<OAuthResponse>)> {
    let Some(session_cookie) = cookies.private(&state.key).get("session") else {
        return Err(error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Authentication required",
        ));
    };

    let Ok(session_id) = uuid::Uuid::parse_str(session_cookie.value()) else {
        return Err(error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Invalid session",
        ));
    };

    let now = Utc::now().naive_utc();
    let mut conn = state.db_pool.get().map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session_error",
            "Failed to get DB connection",
        )
    })?;

    let is_admin = tokio::task::spawn_blocking(move || {
        user_sessions::table
            .inner_join(users::table.on(users::id.eq(user_sessions::user_id)))
            .filter(user_sessions::id.eq(session_id))
            .filter(user_sessions::revoked_at.is_null())
            .filter(user_sessions::expires_at.gt(now))
            .select(users::is_admin)
            .first::<bool>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session_error",
            "Failed to verify admin session",
        )
    })
    .and_then(|r| {
        r.map_err(|_| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "session_error",
                "Failed to verify admin session",
            )
        })
    })?;

    if is_admin != Some(true) {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Admin privileges required",
        ));
    }

    Ok(())
}

async fn create_admin_user_from_oauth(
    state: &AppState,
    provider_id: uuid::Uuid,
    provider_user_id: String,
    provider_email: Option<String>,
    desired_username: Option<String>,
    token_response: OAuthTokenResponse,
) -> Result<uuid::Uuid, (StatusCode, Json<OAuthResponse>)> {
    let Some(provider_email) = provider_email.map(|v| v.trim().to_lowercase()) else {
        return Err(error_response(
            StatusCode::BAD_GATEWAY,
            "email_missing",
            "OAuth provider did not return an email address for bootstrap setup",
        ));
    };

    let username = derive_bootstrap_username(desired_username, &provider_email);
    if username.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_username",
            "A valid username is required for bootstrap OAuth setup",
        ));
    }

    let encryption_key =
        encryption_key_from_config_secret(&state.config.secret).map_err(|msg| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "encryption_key_invalid",
                msg.as_str(),
            )
        })?;

    let access_token_encrypted = encrypt_secret(&token_response.access_token, &encryption_key)
        .map_err(|msg| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "encryption_failed",
                msg.as_str(),
            )
        })?;
    let refresh_token_encrypted = token_response
        .refresh_token
        .as_deref()
        .map(|value| encrypt_secret(value, &encryption_key))
        .transpose()
        .map_err(|msg| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "encryption_failed",
                msg.as_str(),
            )
        })?;
    let token_expires_at = token_response
        .expires_in
        .map(|seconds| Utc::now() + chrono::Duration::seconds(seconds))
        .map(|v| v.naive_utc());
    let random_password_hash = hash_password(&random_urlsafe(48)).map_err(|msg| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "password_hash_failed",
            msg.as_str(),
        )
    })?;

    let now = Utc::now().naive_utc();
    let mut conn = state.db_pool.get().map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "setup_error",
            "Failed to get DB connection",
        )
    })?;

    tokio::task::spawn_blocking(move || -> Result<uuid::Uuid, String> {
        let admin_exists = users::table
            .filter(users::is_admin.eq(true))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to query existing admins".to_string())?;
        if admin_exists.is_some() {
            return Err("Setup has already been completed".to_string());
        }

        let existing_user_by_email = users::table
            .filter(users::email.eq(&provider_email))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to validate provider email".to_string())?;
        if existing_user_by_email.is_some() {
            return Err("Email is already in use".to_string());
        }

        let existing_user_by_username = users::table
            .filter(users::username.eq(&username))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to validate username".to_string())?;
        if existing_user_by_username.is_some() {
            return Err("Username is already in use".to_string());
        }

        let existing_link = user_oauth_accounts::table
            .filter(user_oauth_accounts::provider_id.eq(provider_id))
            .filter(user_oauth_accounts::provider_user_id.eq(&provider_user_id))
            .select(UserOAuthAccount::as_select())
            .first::<UserOAuthAccount>(&mut conn)
            .optional()
            .map_err(|_| "Failed to validate OAuth link".to_string())?;
        if existing_link.is_some() {
            return Err("OAuth account is already linked".to_string());
        }

        let user_id = uuid::Uuid::now_v7();
        diesel::insert_into(users::table)
            .values((
                users::id.eq(user_id),
                users::username.eq(username),
                users::email.eq(provider_email.clone()),
                users::is_admin.eq(true),
                users::password_hash.eq(random_password_hash),
                users::created_at.eq(now),
                users::updated_at.eq(now),
                users::two_factor_enabled.eq(false),
                users::two_factor_secret_encrypted.eq::<Option<String>>(None),
                users::two_factor_confirmed_at.eq::<Option<chrono::NaiveDateTime>>(None),
                users::two_factor_admin_required.eq(false),
            ))
            .execute(&mut conn)
            .map_err(|_| "Failed to create admin user".to_string())?;

        let oauth_account = NewUserOAuthAccount {
            id: uuid::Uuid::now_v7(),
            user_id,
            provider_id,
            provider_user_id,
            provider_email: Some(provider_email),
            access_token_encrypted: Some(access_token_encrypted),
            refresh_token_encrypted,
            token_expires_at,
            linked_at: now,
            created_at: now,
            updated_at: now,
        };

        diesel::insert_into(user_oauth_accounts::table)
            .values(&oauth_account)
            .execute(&mut conn)
            .map_err(|_| "Failed to link OAuth account".to_string())?;

        mark_admin_setup_complete_sync(&mut conn, now)?;

        Ok(user_id)
    })
    .await
    .map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "setup_error",
            "Failed to spawn blocking task",
        )
    })
    .and_then(|r| match r {
        Ok(user_id) => Ok(user_id),
        Err(msg) if msg == "Setup has already been completed" => Err(error_response(
            StatusCode::CONFLICT,
            "setup_complete",
            msg.as_str(),
        )),
        Err(msg)
            if msg == "Email is already in use"
                || msg == "Username is already in use"
                || msg == "OAuth account is already linked" =>
        {
            Err(error_response(
                StatusCode::CONFLICT,
                "setup_conflict",
                msg.as_str(),
            ))
        }
        Err(msg) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "setup_error",
            msg.as_str(),
        )),
    })
}

fn mark_admin_setup_complete_sync(
    conn: &mut diesel::PgConnection,
    now: chrono::NaiveDateTime,
) -> Result<(), String> {
    diesel::update(admin_bootstrap_setup::table.filter(admin_bootstrap_setup::id.eq(1)))
        .set((
            admin_bootstrap_setup::setup_completed_at.eq(Some(now)),
            admin_bootstrap_setup::admin_code_hash.eq::<Option<String>>(None),
            admin_bootstrap_setup::admin_code_generated_at
                .eq::<Option<chrono::NaiveDateTime>>(None),
            admin_bootstrap_setup::updated_at.eq(now),
        ))
        .execute(conn)
        .map_err(|_| "Failed to update bootstrap setup state".to_string())?;

    Ok(())
}

fn hash_password(value: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(value.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| format!("Failed to hash value: {e}"))
}

async fn authenticate_password_login(
    state: &AppState,
    cookies: &Cookies,
    identifier: &str,
    password: &str,
    two_factor_code: Option<&str>,
) -> (StatusCode, Json<AuthSessionResponse>) {
    let identifier = identifier.trim().to_string();
    let email_candidate = identifier.to_lowercase();
    if identifier.is_empty() || password.is_empty() {
        return auth_session_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Email or username and password are required",
        );
    }

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let outcome = tokio::task::spawn_blocking(move || {
        users::table
            .filter(
                users::username
                    .eq(&identifier)
                    .or(users::email.eq(&email_candidate)),
            )
            .select((
                users::id,
                users::password_hash,
                users::two_factor_enabled,
                users::two_factor_admin_required,
            ))
            .first::<(uuid::Uuid, String, bool, bool)>(&mut conn)
            .optional()
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query DB");

    let Some((user_id, password_hash, two_factor_enabled, two_factor_admin_required)) = outcome
    else {
        return auth_session_error(StatusCode::UNAUTHORIZED, "failure", "Invalid credentials");
    };

    let parsed_hash = PasswordHash::new(&password_hash).expect("Invalid hash in db");
    if Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return auth_session_error(StatusCode::UNAUTHORIZED, "failure", "Invalid credentials");
    }

    let global_required = match load_global_two_factor_requirement(state).await {
        Ok(value) => value,
        Err(message) => {
            return auth_session_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "settings_error",
                message.as_str(),
            );
        }
    };

    let two_factor_required = global_required || two_factor_admin_required || two_factor_enabled;
    if two_factor_required && two_factor_code.is_none() {
        let challenge_id = match create_login_two_factor_challenge(state, user_id).await {
            Ok(value) => value,
            Err(message) => {
                return auth_session_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "two_factor_error",
                    message.as_str(),
                );
            }
        };

        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthSessionResponse {
                status: "two_factor_required",
                message: "2FA code required to complete sign-in".to_string(),
                session: None,
                login_two_factor_challenge_id: Some(challenge_id),
                requires_email_verification: false,
            }),
        );
    }

    if let Some(code) = two_factor_code {
        match verify_two_factor_for_user(state, user_id, code).await {
            Ok(true) => {}
            Ok(false) => {
                return auth_session_error(
                    StatusCode::UNAUTHORIZED,
                    "two_factor_invalid",
                    "Invalid 2FA code",
                );
            }
            Err(message) => {
                return auth_session_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "two_factor_error",
                    message.as_str(),
                );
            }
        }
    }

    let session = match issue_session_cookie(state, cookies, user_id).await {
        Ok(value) => value,
        Err(message) => {
            return auth_session_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "session_error",
                message.as_str(),
            );
        }
    };

    match build_session_response(
        state,
        user_id,
        session.expires_at,
        "success",
        "Authenticated successfully",
    )
    .await
    {
        Ok(response) => (StatusCode::OK, Json(response)),
        Err(message) => auth_session_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session_error",
            message.as_str(),
        ),
    }
}

async fn build_session_response(
    state: &AppState,
    user_id: uuid::Uuid,
    session_expires_at: chrono::NaiveDateTime,
    status: &'static str,
    message: &str,
) -> Result<AuthSessionResponse, String> {
    let user = load_public_user(state, user_id).await?;
    Ok(AuthSessionResponse {
        status,
        message: message.to_string(),
        session: Some(SessionStateResponse {
            authenticated: true,
            user,
            session_expires_at,
        }),
        login_two_factor_challenge_id: None,
        requires_email_verification: false,
    })
}

fn auth_session_error(
    status_code: StatusCode,
    status: &'static str,
    message: &str,
) -> (StatusCode, Json<AuthSessionResponse>) {
    (
        status_code,
        Json(AuthSessionResponse {
            status,
            message: message.to_string(),
            session: None,
            login_two_factor_challenge_id: None,
            requires_email_verification: false,
        }),
    )
}

fn code_challenge_from_verifier(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

pub(super) fn random_urlsafe(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    let mut rng = rand::rng();
    rng.fill(&mut bytes[..]);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn encryption_key_from_config_secret(secret_b64: &str) -> Result<[u8; 32], String> {
    let secret_bytes = general_purpose::STANDARD
        .decode(secret_b64)
        .map_err(|_| "Config secret is not valid base64".to_string())?;
    let digest = Sha256::digest(secret_bytes);
    let mut key = [0u8; 32];
    key.copy_from_slice(&digest[..32]);
    Ok(key)
}

fn encrypt_secret(plaintext: &str, key: &[u8; 32]) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("Cipher init failed: {e}"))?;
    let mut nonce_bytes = [0u8; 12];
    let mut rng = rand::rng();
    rng.fill(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encryption failed: {e}"))?;

    let nonce_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(nonce_bytes);
    let cipher_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(ciphertext);
    Ok(format!("v1.{nonce_b64}.{cipher_b64}"))
}

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

