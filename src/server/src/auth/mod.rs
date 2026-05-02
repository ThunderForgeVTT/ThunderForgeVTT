use crate::models::{
    AuthSecuritySetting, LoginTwoFactorChallenge, NewLoginTwoFactorChallenge,
    NewOAuthAuthorizationSession, NewOAuthLinkChallenge, NewUserOAuthAccount,
    OAuthAuthorizationSession, OAuthLinkChallenge, OAuthProvider, UserOAuthAccount,
};
use crate::schema::{
    auth_security_settings, login_two_factor_challenges, oauth_authorization_sessions,
    oauth_link_challenges, oauth_providers, user_oauth_accounts, users,
};
use crate::state::AppState;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
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
use tower_cookies::{Cookie, Cookies};
use url::Url;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/authentication/basic", post(basic_authentication))
        .route("/authentication/oauth/resolve", post(oauth_resolve))
        .route(
            "/authentication/oauth/link/confirm",
            post(oauth_link_confirm),
        )
        .route(
            "/authentication/oauth/:provider_key/start",
            get(oauth_start),
        )
        .route(
            "/authentication/oauth/:provider_key/callback",
            get(oauth_callback),
        )
        .route(
            "/authentication/oauth/:provider_key/token",
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
            "/authentication/admin/users/:user_id/2fa/required",
            post(set_admin_user_two_factor_required),
        )
        .route("/authentication/logout", post(logout))
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
struct OAuthResponse {
    status: &'static str,
    message: String,
    challenge_id: Option<uuid::Uuid>,
    login_two_factor_challenge_id: Option<uuid::Uuid>,
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

async fn basic_authentication(
    cookies: Cookies,
    headers: HeaderMap,
    State(state): State<AppState>,
    credentials: String,
) -> (StatusCode, Json<OAuthResponse>) {
    println!("{}", &credentials);
    let cred = Credentials::from(credentials);

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");

    let outcome = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::username.eq(&cred.username))
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
        return error_response(StatusCode::UNAUTHORIZED, "failure", "Invalid credentials");
    };

    let parsed_hash = PasswordHash::new(&password_hash).expect("Invalid hash in db");
    if Argon2::default()
        .verify_password(cred.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return error_response(StatusCode::UNAUTHORIZED, "failure", "Invalid credentials");
    }

    let global_required = match load_global_two_factor_requirement(&state).await {
        Ok(v) => v,
        Err(msg) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "settings_error",
                msg.as_str(),
            );
        }
    };

    let two_factor_required = global_required || two_factor_admin_required || two_factor_enabled;

    if !two_factor_required {
        let mut cookie = Cookie::new("session", user_id.to_string());
        cookie.set_path("/");
        cookies.private(&state.key).add(cookie);
        return (
            StatusCode::OK,
            Json(OAuthResponse {
                status: "success",
                message: "Authenticated successfully".to_string(),
                challenge_id: None,
                login_two_factor_challenge_id: None,
            }),
        );
    }

    let Some(code) = headers
        .get("x-2fa-code")
        .and_then(|h| h.to_str().ok())
        .map(|v| v.to_string())
    else {
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
    };

    match verify_two_factor_for_user(&state, user_id, &code).await {
        Ok(true) => {
            let mut cookie = Cookie::new("session", user_id.to_string());
            cookie.set_path("/");
            cookies.private(&state.key).add(cookie);
            (
                StatusCode::OK,
                Json(OAuthResponse {
                    status: "success",
                    message: "Authenticated successfully".to_string(),
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

            let mut cookie = Cookie::new("session", user_id.to_string());
            cookie.set_path("/");
            cookies.private(&state.key).add(cookie);

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
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<AdminTwoFactorRequirementRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    if let Err(resp) = verify_admin_request(&headers) {
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
    headers: HeaderMap,
    Path(user_id): Path<uuid::Uuid>,
    State(state): State<AppState>,
    Json(request): Json<AdminUserTwoFactorRequiredRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    if let Err(resp) = verify_admin_request(&headers) {
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
async fn logout(cookies: Cookies, State(state): State<AppState>) {
    cookies
        .private(&state.key)
        .remove(Cookie::new("session", ""));
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
        .append_pair("scope", &provider.scopes.join(" "))
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
    let provider_user_id = request.provider_user_id;
    let provider_email = request.provider_email;
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
                return Ok(ResolveOutcome::NoMatchingUser);
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

            let mut cookie = Cookie::new("session", user_id.to_string());
            cookie.set_path("/");
            cookies.private(&state.key).add(cookie);

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
        ResolveOutcome::PasswordRequired(challenge_id) => (
            StatusCode::CONFLICT,
            Json(OAuthResponse {
                status: "password_required",
                message: "Existing account detected; confirm password to link this OAuth account"
                    .to_string(),
                challenge_id: Some(challenge_id),
                login_two_factor_challenge_id: None,
            }),
        ),
        ResolveOutcome::NoMatchingUser => error_response(
            StatusCode::NOT_FOUND,
            "no_matching_user",
            "No existing user matched this OAuth identity",
        ),
    }
}

async fn oauth_link_confirm(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<OAuthLinkConfirmRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
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

            if let Some(account_for_subject) = account_for_subject.as_ref() {
                if account_for_subject.user_id != challenge.user_id {
                    return Ok(LinkConfirmOutcome::LinkConflict);
                }
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
            let mut cookie = Cookie::new("session", user_id.to_string());
            cookie.set_path("/");
            cookies.private(&state.key).add(cookie);

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
    let Some(client_id) = auth_ctx.provider.oauth_client_id.as_deref() else {
        return Err("Provider client id is not configured".to_string());
    };
    let Some(client_secret) = auth_ctx.provider.oauth_client_secret.as_deref() else {
        return Err("Provider client secret is not configured".to_string());
    };

    let client = reqwest::Client::new();
    let response = client
        .post(&auth_ctx.provider.token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &auth_ctx.session.redirect_uri),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code_verifier", &auth_ctx.session.code_verifier),
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

fn verify_admin_request(headers: &HeaderMap) -> Result<(), (StatusCode, Json<OAuthResponse>)> {
    let configured_secret = match std::env::var("ADMIN_SECRET") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            return Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "admin_secret_missing",
                "ADMIN_SECRET is not configured",
            ));
        }
    };

    let provided = headers
        .get("x-admin-secret")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();

    if provided != configured_secret {
        return Err(error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Missing or invalid admin secret",
        ));
    }
    Ok(())
}

fn code_challenge_from_verifier(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

fn random_urlsafe(len: usize) -> String {
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
