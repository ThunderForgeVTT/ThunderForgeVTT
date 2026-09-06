//! Passwords and sessions: logging in, registering, reading and refreshing a
//! session, logging out, and the cookie that carries it.

use super::*;

pub(crate) async fn basic_authentication(
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

pub(crate) async fn login(
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

pub(crate) async fn register(
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

pub(crate) async fn current_session(
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

pub(crate) async fn refresh_session(
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

pub(crate) async fn logout(
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

pub(crate) async fn issue_session_cookie(
    state: &AppState,
    cookies: &Cookies,
    user_id: uuid::Uuid,
) -> Result<crate::models::UserSession, String> {
    let now = Utc::now().naive_utc();
    let session_id = uuid::Uuid::now_v7();
    let expires_at = now + chrono::Duration::days(session::SESSION_TTL_DAYS);
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

    // The session cookie is encrypted (`.private`); the CSRF cookie is not,
    // because the front end has to be able to read it back. Both shapes are
    // decided in `thunderforge_axum_auth_core::session` so there is one place
    // to look for "is HttpOnly set on that one?".
    cookies
        .private(&state.key)
        .add(cookie_from_spec(session_cookie(
            &session_id.to_string(),
            state.config.secure_cookies,
        )));
    cookies.add(cookie_from_spec(csrf_cookie(
        &uuid::Uuid::now_v7().to_string(),
        state.config.secure_cookies,
    )));
    Ok(crate::models::UserSession {
        id: session_id,
        user_id,
        expires_at,
        revoked_at: None,
        created_at: now,
    })
}

pub(crate) fn hash_password(value: &str) -> Result<String, String> {
    Argon2::default()
        .hash_password(value.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|e| format!("Failed to hash value: {e}"))
}

pub(crate) async fn authenticate_password_login(
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

pub(crate) async fn build_session_response(
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

pub(crate) fn auth_session_error(
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
